use std::time::Instant;
use std::{cell::RefCell, process::Command, rc::Rc, sync::Arc};
use std::{f32, os::raw::c_char};

use ash::{khr, vk};
use lahar::DedicatedImage;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tracing::{error, info};
use winit::event::{KeyEvent, MouseScrollDelta};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    window::{CursorGrabMode, Fullscreen, Window as WinitWindow},
};

use super::{Base, Core, Draw, Frustum};
use super::{gui::GuiState, material_icons::MaterialIcons};
use crate::{Action, Config, SettingsStore, Sim, WorldManager, settings::VideoSettings};

/// OS window
pub struct EarlyWindow {
    window: WinitWindow,
    required_extensions: &'static [*const c_char],
}

impl EarlyWindow {
    pub fn new(event_loop: &ActiveEventLoop, video: &VideoSettings) -> Self {
        let mut attrs = WinitWindow::default_attributes()
            .with_title("Hypermine")
            .with_inner_size(PhysicalSize::new(video.width, video.height));
        if video.fullscreen {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let window = event_loop.create_window(attrs).unwrap();
        Self {
            window,
            required_extensions: ash_window::enumerate_required_extensions(
                event_loop.display_handle().unwrap().as_raw(),
            )
            .expect("unsupported platform"),
        }
    }

    /// Identify the Vulkan extension needed to render to this window
    pub fn required_extensions(&self) -> &'static [*const c_char] {
        self.required_extensions
    }
}

/// OS window + rendering handles
pub struct Window {
    _core: Arc<Core>,
    pub window: WinitWindow,
    config: Arc<Config>,
    surface_fn: khr::surface::Instance,
    surface: vk::SurfaceKHR,
    swapchain: Option<SwapchainMgr>,
    swapchain_needs_update: bool,
    draw: Option<Draw>,
    sim: Option<Sim>,
    gui_state: GuiState,
    yak: yakui::Yakui,
    net: server::Handle,
    settings: Rc<RefCell<SettingsStore>>,
    worlds: Rc<RefCell<WorldManager>>,
    input: InputState,
    last_frame: Option<Instant>,
}

impl Window {
    /// Finish constructing a window
    pub fn new(
        early: EarlyWindow,
        core: Arc<Core>,
        config: Arc<Config>,
        net: server::Handle,
        settings: Rc<RefCell<SettingsStore>>,
        worlds: Rc<RefCell<WorldManager>>,
    ) -> Self {
        let surface = unsafe {
            ash_window::create_surface(
                &core.entry,
                &core.instance,
                early.window.display_handle().unwrap().as_raw(),
                early.window.window_handle().unwrap().as_raw(),
                None,
            )
            .unwrap()
        };
        let surface_fn = khr::surface::Instance::new(&core.entry, &core.instance);
        let mut yak = yakui::Yakui::new();
        let icons = MaterialIcons::load(&mut yak, &config);

        Self {
            _core: core,
            window: early.window,
            config,
            surface,
            surface_fn,
            swapchain: None,
            swapchain_needs_update: false,
            draw: None,
            sim: None,
            gui_state: GuiState::new(icons),
            yak,
            net,
            settings,
            worlds,
            input: InputState::default(),
            last_frame: None,
        }
    }

    /// Determine whether this window can be rendered to from a particular device and queue family
    pub fn supports(&self, physical: vk::PhysicalDevice, queue_family_index: u32) -> bool {
        unsafe {
            self.surface_fn
                .get_physical_device_surface_support(physical, queue_family_index, self.surface)
                .unwrap()
        }
    }

    pub fn init_rendering(&mut self, gfx: Arc<Base>) {
        // Allocate the presentable images we'll be rendering to
        let vsync = self.settings.borrow().value.video.vsync;
        self.swapchain = Some(SwapchainMgr::new(
            self,
            gfx.clone(),
            self.window.inner_size(),
            vsync,
        ));
        // Construct the core rendering object
        self.draw = Some(Draw::new(gfx, self.config.clone()));
    }

    pub fn handle_device_event(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::MouseMotion { delta }
                if self.input.mouse_captured && !self.gui_state.menu_open() =>
            {
                if let Some(sim) = self.sim.as_mut() {
                    let controls = &self.settings.borrow().value.controls;
                    let sensitivity = 2e-3 * controls.mouse_sensitivity;
                    let pitch = if controls.invert_y { delta.1 } else { -delta.1 };
                    sim.look(
                        -delta.0 as f32 * sensitivity,
                        pitch as f32 * sensitivity,
                        0.0,
                    );
                }
            }
            _ => {}
        }
    }

    pub fn handle_event(&mut self, event: WindowEvent, event_loop: &ActiveEventLoop) {
        match event {
            WindowEvent::RedrawRequested => {
                while let Ok(msg) = self.net.incoming.try_recv() {
                    self.handle_net(msg);
                }

                let this_frame = Instant::now();
                if self.gui_state.menu_open() {
                    self.input.clear_motion();
                    self.last_frame = Some(this_frame);
                } else if let Some(sim) = self.sim.as_mut() {
                    let dt = this_frame - self.last_frame.unwrap_or(this_frame);
                    sim.set_movement_input(self.input.movement());
                    sim.set_jump_held(self.input.jump);
                    sim.look(0.0, 0.0, 2.0 * self.input.roll() * dt.as_secs_f32());
                    sim.step(dt, &mut self.net);
                    self.last_frame = Some(this_frame);
                }

                self.draw(event_loop);
            }
            WindowEvent::Resized(_) => {
                // Some environments may not emit the vulkan signals that recommend or
                // require surface reconstruction, so we need to check for messages from the
                // windowing system too. We defer actually performing the update until
                // drawing to avoid doing unnecessary work between frames.
                self.swapchain_needs_update = true;
            }
            WindowEvent::CloseRequested => {
                info!("exiting due to closed window");
                event_loop.exit();
            }
            WindowEvent::CursorMoved { position, .. } if self.gui_state.menu_open() => {
                self.gui_state
                    .set_cursor_position([position.x as f32, position.y as f32]);
                self.yak.handle_event(yakui::event::Event::CursorMoved(Some(
                    [position.x as f32, position.y as f32].into(),
                )));
            }
            WindowEvent::CursorLeft { .. } if self.gui_state.menu_open() => {
                self.yak
                    .handle_event(yakui::event::Event::CursorMoved(None));
            }
            WindowEvent::MouseWheel { delta, .. } if self.gui_state.menu_open() => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => [x * 40.0, y * 40.0].into(),
                    MouseScrollDelta::PixelDelta(position) => {
                        [position.x as f32, position.y as f32].into()
                    }
                };
                self.yak
                    .handle_event(yakui::event::Event::MouseScroll { delta });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let vertical = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32,
                };
                if vertical != 0.0 {
                    self.cycle_hotbar(if vertical > 0.0 { -1 } else { 1 });
                }
            }
            WindowEvent::MouseInput { button, state, .. } if self.gui_state.menu_open() => {
                if let Some(button) = yak_mouse_button(button) {
                    self.yak
                        .handle_event(yakui::event::Event::MouseButtonChanged {
                            button,
                            down: state == ElementState::Pressed,
                        });
                }
            }
            WindowEvent::MouseInput {
                button,
                state: ElementState::Pressed,
                ..
            } => {
                if !self.input.mouse_captured {
                    self.capture_cursor();
                } else if let Some(sim) = self.sim.as_mut() {
                    match button {
                        MouseButton::Left => sim.set_break_block_pressed_true(),
                        MouseButton::Right => sim.set_place_block_pressed_true(),
                        _ => {}
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(key),
                        text,
                        repeat,
                        ..
                    },
                ..
            } => {
                let pressed = state == ElementState::Pressed;
                if key == KeyCode::Escape && pressed && !repeat {
                    if self.gui_state.inventory_open() {
                        let world_id = self.worlds.borrow().selected_id().to_owned();
                        self.gui_state.close_inventory(
                            self.sim.as_mut(),
                            &mut self.settings.borrow_mut(),
                            &world_id,
                        );
                    } else {
                        self.gui_state.handle_escape();
                    }
                    if self.gui_state.menu_open() {
                        self.release_cursor();
                    } else {
                        self.capture_cursor();
                    }
                    return;
                }

                if self.gui_state.menu_open() {
                    if self.gui_state.is_rebinding() && pressed && !repeat {
                        self.gui_state
                            .capture_binding(format!("{key:?}"), &mut self.settings.borrow_mut());
                        return;
                    }
                    if let Some(key) = yak_key(key) {
                        self.yak
                            .handle_event(yakui::event::Event::KeyChanged { key, down: pressed });
                    }
                    if pressed && let Some(text) = text {
                        for character in text.chars().filter(|character| !character.is_control()) {
                            self.yak
                                .handle_event(yakui::event::Event::TextInput(character));
                        }
                    }
                    return;
                }

                self.handle_bound_key(&format!("{key:?}"), pressed, repeat);
            }
            WindowEvent::Focused(false) => {
                self.release_cursor();
                self.input.clear_motion();
            }
            _ => {}
        }
    }

    fn handle_bound_key(&mut self, key: &str, pressed: bool, repeat: bool) {
        let controls = self.settings.borrow().value.controls.clone();
        let bound = |action| controls.binding(action) == key;
        if bound(Action::Forward) {
            self.input.forward = pressed;
        }
        if bound(Action::Backward) {
            self.input.back = pressed;
        }
        if bound(Action::Left) {
            self.input.left = pressed;
        }
        if bound(Action::Right) {
            self.input.right = pressed;
        }
        if bound(Action::MoveUp) {
            self.input.up = pressed;
        }
        if bound(Action::MoveDown) {
            self.input.down = pressed;
        }
        if bound(Action::RollLeft) {
            self.input.anticlockwise = pressed;
        }
        if bound(Action::RollRight) {
            self.input.clockwise = pressed;
        }
        if bound(Action::Jump) {
            if pressed
                && !self.input.jump
                && let Some(sim) = self.sim.as_mut()
            {
                sim.set_jump_pressed_true();
            }
            self.input.jump = pressed;
        }

        if !pressed || repeat {
            return;
        }
        if bound(Action::ToggleNoClip)
            && let Some(sim) = self.sim.as_mut()
        {
            sim.toggle_no_clip();
        }
        if bound(Action::ToggleHud) {
            self.gui_state.toggle_hud();
        }
        if bound(Action::DeveloperOverlay) {
            self.gui_state.toggle_debug();
        }
        if bound(Action::OpenInventory) {
            self.open_inventory();
            return;
        }
        if bound(Action::PreviousMaterial) {
            self.cycle_hotbar(-1);
        }
        if bound(Action::NextMaterial) {
            self.cycle_hotbar(1);
        }
        if bound(Action::PickMaterial)
            && let Some(material) = self.sim.as_ref().and_then(Sim::looked_at_material)
        {
            self.pick_material(material);
        }
        for action in Action::ALL {
            if bound(action)
                && let Some(index) = action.material_index()
            {
                self.select_hotbar_slot(index);
            }
        }
    }

    fn open_inventory(&mut self) {
        let world_id = self.worlds.borrow().selected_id().to_owned();
        self.gui_state.open_inventory(
            self.sim.as_mut(),
            &mut self.settings.borrow_mut(),
            &world_id,
        );
        self.release_cursor();
        self.input.clear_motion();
    }

    fn select_hotbar_slot(&mut self, index: usize) {
        let world_id = self.worlds.borrow().selected_id().to_owned();
        self.gui_state.select_hotbar_slot(
            index,
            self.sim.as_mut(),
            &mut self.settings.borrow_mut(),
            &world_id,
        );
    }

    fn cycle_hotbar(&mut self, delta: i32) {
        let world_id = self.worlds.borrow().selected_id().to_owned();
        self.gui_state.cycle_hotbar(
            delta,
            self.sim.as_mut(),
            &mut self.settings.borrow_mut(),
            &world_id,
        );
    }

    fn pick_material(&mut self, material: common::world::Material) {
        let world_id = self.worlds.borrow().selected_id().to_owned();
        self.gui_state.pick_material(
            material,
            self.sim.as_mut(),
            &mut self.settings.borrow_mut(),
            &world_id,
        );
    }

    fn release_cursor(&mut self) {
        let _ = self.window.set_cursor_grab(CursorGrabMode::None);
        self.window.set_cursor_visible(true);
        self.input.mouse_captured = false;
    }

    fn capture_cursor(&mut self) {
        let _ = self
            .window
            .set_cursor_grab(CursorGrabMode::Confined)
            .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Locked));
        self.window.set_cursor_visible(false);
        self.input.mouse_captured = true;
    }

    fn handle_net(&mut self, msg: server::Message) {
        match msg {
            server::Message::ConnectionLost(e) => {
                error!("connection lost: {}", e);
            }
            server::Message::Hello(msg) => {
                let mut sim = Sim::new(
                    msg.sim_config,
                    self.config.chunk_load_parallelism as usize,
                    msg.character,
                );
                apply_video_to_sim(&mut sim, &self.settings.borrow().value.video);
                if let Some(draw) = self.draw.as_mut() {
                    draw.configure(sim.cfg());
                }
                self.sim = Some(sim);
            }
            msg => {
                if let Some(sim) = self.sim.as_mut() {
                    sim.handle_net(msg);
                } else {
                    error!("Received game data before ServerHello");
                }
            }
        }
    }

    /// Draw a new frame
    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let initial_extent = self.swapchain.as_ref().unwrap().state.extent;
        let video = self.settings.borrow().value.video.clone();
        self.yak.set_scale_factor(video.ui_scale);
        self.yak
            .set_surface_size([initial_extent.width as f32, initial_extent.height as f32].into());
        self.yak
            .set_unscaled_viewport(yakui::geometry::Rect::from_pos_size(
                Default::default(),
                [initial_extent.width as f32, initial_extent.height as f32].into(),
            ));
        let menu_was_open = self.gui_state.menu_open();
        self.yak.start();
        let gui_action = self.gui_state.run(
            self.sim.as_mut(),
            &mut self.settings.borrow_mut(),
            &mut self.worlds.borrow_mut(),
            [initial_extent.width as f32, initial_extent.height as f32],
        );
        self.yak.finish();

        if gui_action.video_changed {
            self.apply_video_settings();
        }
        if menu_was_open && !self.gui_state.menu_open() {
            self.capture_cursor();
        }
        if gui_action.quit {
            event_loop.exit();
            return;
        }
        if gui_action.restart {
            match std::env::current_exe().and_then(|exe| {
                let mut command = Command::new(exe);
                if let Ok(directory) = std::env::current_dir() {
                    command.current_dir(directory);
                }
                command.spawn().map(|_| ())
            }) {
                Ok(()) => event_loop.exit(),
                Err(error) => error!("couldn't restart for world switch: {error}"),
            }
            return;
        }

        let swapchain = self.swapchain.as_mut().unwrap();
        let draw = self.draw.as_mut().unwrap();
        unsafe {
            // Wait for a frame's worth of rendering resources to become available
            draw.wait();
            // Get the index of the swapchain image we'll render to
            let frame_id = loop {
                // Check whether the window has been resized or similar
                if self.swapchain_needs_update {
                    // Wait for all in-flight frames to complete so we don't have a use-after-free
                    draw.wait_idle();
                    // Recreate the swapchain at a new size (or whatever)
                    swapchain.update(
                        &self.surface_fn,
                        self.surface,
                        self.window.inner_size(),
                        self.settings.borrow().value.video.vsync,
                    );
                    self.swapchain_needs_update = false;
                }
                match swapchain.acquire_next_image(draw.image_acquired()) {
                    Ok((idx, suboptimal)) => {
                        self.swapchain_needs_update = suboptimal;
                        break idx;
                    }
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        self.swapchain_needs_update = true;
                    }
                    Err(e) => {
                        panic!("acquire_next_image: {e}");
                    }
                }
            };
            let extent = swapchain.state.extent;
            let aspect_ratio = extent.width as f32 / extent.height as f32;
            let frame = &swapchain.state.frames[frame_id as usize];
            let vfov = self.settings.borrow().value.video.fov_degrees.to_radians();
            let frustum = Frustum::from_vfov(vfov, aspect_ratio);
            // Render the frame
            draw.draw(
                self.sim.as_mut(),
                self.yak.paint(),
                frame.buffer,
                frame.depth_view,
                extent,
                frame.present,
                &frustum,
            );
            // Submit the frame to be presented on the window
            match swapchain.queue_present(frame_id) {
                Ok(false) => {}
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.swapchain_needs_update = true;
                }
                Err(e) => panic!("queue_present: {e}"),
            };
        }
    }

    fn apply_video_settings(&mut self) {
        let video = self.settings.borrow().value.video.clone();
        apply_video_to_sim_option(self.sim.as_mut(), &video);
        self.window.set_fullscreen(if video.fullscreen {
            Some(Fullscreen::Borderless(self.window.current_monitor()))
        } else {
            None
        });
        if !video.fullscreen {
            let _ = self
                .window
                .request_inner_size(PhysicalSize::new(video.width, video.height));
        }
        self.swapchain_needs_update = true;
    }
}

fn apply_video_to_sim_option(sim: Option<&mut Sim>, video: &VideoSettings) {
    if let Some(sim) = sim {
        apply_video_to_sim(sim, video);
    }
}

fn apply_video_to_sim(sim: &mut Sim, video: &VideoSettings) {
    let scale = sim.cfg.meters_to_absolute;
    sim.cfg.view_distance = video.view_distance_m * scale;
    sim.cfg.chunk_generation_distance = (video.view_distance_m - 10.0).max(25.0) * scale;
    sim.cfg.fog_distance = (video.view_distance_m + 15.0) * scale;
}

fn yak_mouse_button(button: MouseButton) -> Option<yakui::input::MouseButton> {
    match button {
        MouseButton::Left => Some(yakui::input::MouseButton::One),
        MouseButton::Right => Some(yakui::input::MouseButton::Two),
        MouseButton::Middle => Some(yakui::input::MouseButton::Three),
        _ => None,
    }
}

fn yak_key(key: KeyCode) -> Option<yakui::input::KeyCode> {
    use yakui::input::KeyCode as Yak;
    Some(match key {
        KeyCode::KeyA => Yak::KeyA,
        KeyCode::KeyB => Yak::KeyB,
        KeyCode::KeyC => Yak::KeyC,
        KeyCode::KeyD => Yak::KeyD,
        KeyCode::KeyE => Yak::KeyE,
        KeyCode::KeyF => Yak::KeyF,
        KeyCode::KeyG => Yak::KeyG,
        KeyCode::KeyH => Yak::KeyH,
        KeyCode::KeyI => Yak::KeyI,
        KeyCode::KeyJ => Yak::KeyJ,
        KeyCode::KeyK => Yak::KeyK,
        KeyCode::KeyL => Yak::KeyL,
        KeyCode::KeyM => Yak::KeyM,
        KeyCode::KeyN => Yak::KeyN,
        KeyCode::KeyO => Yak::KeyO,
        KeyCode::KeyP => Yak::KeyP,
        KeyCode::KeyQ => Yak::KeyQ,
        KeyCode::KeyR => Yak::KeyR,
        KeyCode::KeyS => Yak::KeyS,
        KeyCode::KeyT => Yak::KeyT,
        KeyCode::KeyU => Yak::KeyU,
        KeyCode::KeyV => Yak::KeyV,
        KeyCode::KeyW => Yak::KeyW,
        KeyCode::KeyX => Yak::KeyX,
        KeyCode::KeyY => Yak::KeyY,
        KeyCode::KeyZ => Yak::KeyZ,
        KeyCode::Digit0 => Yak::Digit0,
        KeyCode::Digit1 => Yak::Digit1,
        KeyCode::Digit2 => Yak::Digit2,
        KeyCode::Digit3 => Yak::Digit3,
        KeyCode::Digit4 => Yak::Digit4,
        KeyCode::Digit5 => Yak::Digit5,
        KeyCode::Digit6 => Yak::Digit6,
        KeyCode::Digit7 => Yak::Digit7,
        KeyCode::Digit8 => Yak::Digit8,
        KeyCode::Digit9 => Yak::Digit9,
        KeyCode::Space => Yak::Space,
        KeyCode::Enter => Yak::Enter,
        KeyCode::Backspace => Yak::Backspace,
        KeyCode::Delete => Yak::Delete,
        KeyCode::ArrowLeft => Yak::ArrowLeft,
        KeyCode::ArrowRight => Yak::ArrowRight,
        KeyCode::ArrowUp => Yak::ArrowUp,
        KeyCode::ArrowDown => Yak::ArrowDown,
        KeyCode::Home => Yak::Home,
        KeyCode::End => Yak::End,
        KeyCode::Tab => Yak::Tab,
        _ => return None,
    })
}

impl Drop for Window {
    fn drop(&mut self) {
        self.draw.take();
        self.swapchain.take();
        unsafe {
            self.surface_fn.destroy_surface(self.surface, None);
        }
    }
}

struct SwapchainMgr {
    state: SwapchainState,
    format: vk::SurfaceFormatKHR,
}

impl SwapchainMgr {
    /// Construct a swapchain manager for a certain window
    fn new(window: &Window, gfx: Arc<Base>, fallback_size: PhysicalSize<u32>, vsync: bool) -> Self {
        let device = &*gfx.device;
        let swapchain_fn = khr::swapchain::Device::new(&gfx.core.instance, device);
        let surface_formats = unsafe {
            window
                .surface_fn
                .get_physical_device_surface_formats(gfx.physical, window.surface)
                .unwrap()
        };
        let desired_format = vk::SurfaceFormatKHR {
            format: super::base::COLOR_FORMAT,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        };

        let desirable_format = |x: &vk::SurfaceFormatKHR| -> bool {
            x.format == desired_format.format && x.color_space == desired_format.color_space
        };

        if (surface_formats.len() != 1
            || (surface_formats[0].format != vk::Format::UNDEFINED
                || surface_formats[0].color_space != desired_format.color_space))
            && !surface_formats.iter().any(desirable_format)
        {
            panic!("no suitable surface format: {surface_formats:?}");
        }

        Self {
            state: unsafe {
                SwapchainState::new(
                    &window.surface_fn,
                    swapchain_fn,
                    gfx,
                    window.surface,
                    desired_format,
                    vk::SwapchainKHR::null(),
                    SwapchainOptions {
                        fallback_size,
                        vsync,
                    },
                )
            },
            format: desired_format,
        }
    }

    /// Recreate the swapchain based on the window's current capabilities
    ///
    /// # Safety
    /// - There must be no operations scheduled that access the current swapchain
    unsafe fn update(
        &mut self,
        surface_fn: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
        fallback_size: PhysicalSize<u32>,
        vsync: bool,
    ) {
        unsafe {
            self.state = SwapchainState::new(
                surface_fn,
                self.state.swapchain_fn.clone(),
                self.state.gfx.clone(),
                surface,
                self.format,
                self.state.handle,
                SwapchainOptions {
                    fallback_size,
                    vsync,
                },
            );
        }
    }

    /// Get the index of the next frame to use
    unsafe fn acquire_next_image(&self, signal: vk::Semaphore) -> Result<(u32, bool), vk::Result> {
        unsafe {
            self.state.swapchain_fn.acquire_next_image(
                self.state.handle,
                u64::MAX,
                signal,
                vk::Fence::null(),
            )
        }
    }

    /// Present a frame on the window
    unsafe fn queue_present(&self, index: u32) -> Result<bool, vk::Result> {
        unsafe {
            self.state.swapchain_fn.queue_present(
                self.state.gfx.queue,
                &vk::PresentInfoKHR::default()
                    .wait_semaphores(&[self.state.frames[index as usize].present])
                    .swapchains(&[self.state.handle])
                    .image_indices(&[index]),
            )
        }
    }
}

/// Data that's replaced when the swapchain is updated
struct SwapchainState {
    gfx: Arc<Base>,
    swapchain_fn: khr::swapchain::Device,
    extent: vk::Extent2D,
    handle: vk::SwapchainKHR,
    frames: Vec<Frame>,
}

struct SwapchainOptions {
    fallback_size: PhysicalSize<u32>,
    vsync: bool,
}

impl SwapchainState {
    unsafe fn new(
        surface_fn: &khr::surface::Instance,
        swapchain_fn: khr::swapchain::Device,
        gfx: Arc<Base>,
        surface: vk::SurfaceKHR,
        format: vk::SurfaceFormatKHR,
        old: vk::SwapchainKHR,
        options: SwapchainOptions,
    ) -> Self {
        unsafe {
            let device = &*gfx.device;

            let surface_capabilities = surface_fn
                .get_physical_device_surface_capabilities(gfx.physical, surface)
                .unwrap();
            let extent = match surface_capabilities.current_extent.width {
                // If Vulkan doesn't know, winit probably does. Known to apply at least to Wayland.
                std::u32::MAX => vk::Extent2D {
                    width: options.fallback_size.width,
                    height: options.fallback_size.height,
                },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_fn
                .get_physical_device_surface_present_modes(gfx.physical, surface)
                .unwrap();
            let present_mode = if options.vsync {
                vk::PresentModeKHR::FIFO
            } else {
                present_modes
                    .iter()
                    .cloned()
                    .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                    .unwrap_or(vk::PresentModeKHR::FIFO)
            };

            let image_count = if surface_capabilities.max_image_count > 0 {
                surface_capabilities
                    .max_image_count
                    .min(surface_capabilities.min_image_count + 1)
            } else {
                surface_capabilities.min_image_count + 1
            };

            let handle = swapchain_fn
                .create_swapchain(
                    &vk::SwapchainCreateInfoKHR::default()
                        .surface(surface)
                        .min_image_count(image_count)
                        .image_color_space(format.color_space)
                        .image_format(format.format)
                        .image_extent(extent)
                        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .pre_transform(pre_transform)
                        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                        .present_mode(present_mode)
                        .clipped(true)
                        .image_array_layers(1)
                        .old_swapchain(old),
                    None,
                )
                .unwrap();

            let frames = swapchain_fn
                .get_swapchain_images(handle)
                .unwrap()
                .into_iter()
                .map(|image| {
                    let view = device
                        .create_image_view(
                            &vk::ImageViewCreateInfo::default()
                                .view_type(vk::ImageViewType::TYPE_2D)
                                .format(format.format)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                    base_mip_level: 0,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: 1,
                                })
                                .image(image),
                            None,
                        )
                        .unwrap();
                    gfx.set_name(view, cstr!("swapchain"));
                    let depth = DedicatedImage::new(
                        device,
                        &gfx.memory_properties,
                        &vk::ImageCreateInfo::default()
                            .image_type(vk::ImageType::TYPE_2D)
                            .format(vk::Format::D32_SFLOAT)
                            .extent(vk::Extent3D {
                                width: extent.width,
                                height: extent.height,
                                depth: 1,
                            })
                            .mip_levels(1)
                            .array_layers(1)
                            .samples(vk::SampleCountFlags::TYPE_1)
                            .usage(
                                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                                    | vk::ImageUsageFlags::INPUT_ATTACHMENT,
                            ),
                    );
                    gfx.set_name(depth.handle, cstr!("depth"));
                    gfx.set_name(depth.memory, cstr!("depth"));
                    let depth_view = device
                        .create_image_view(
                            &vk::ImageViewCreateInfo::default()
                                .image(depth.handle)
                                .view_type(vk::ImageViewType::TYPE_2D)
                                .format(vk::Format::D32_SFLOAT)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                                    base_mip_level: 0,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: 1,
                                }),
                            None,
                        )
                        .unwrap();
                    gfx.set_name(depth_view, cstr!("depth"));
                    let present = device.create_semaphore(&Default::default(), None).unwrap();
                    gfx.set_name(present, cstr!("present"));
                    Frame {
                        view,
                        depth,
                        depth_view,
                        buffer: device
                            .create_framebuffer(
                                &vk::FramebufferCreateInfo::default()
                                    .render_pass(gfx.render_pass)
                                    .attachments(&[view, depth_view])
                                    .width(extent.width)
                                    .height(extent.height)
                                    .layers(1),
                                None,
                            )
                            .unwrap(),
                        present,
                    }
                })
                .collect();

            Self {
                swapchain_fn,
                gfx,
                extent,
                handle,
                frames,
            }
        }
    }
}

impl Drop for SwapchainState {
    fn drop(&mut self) {
        let device = &*self.gfx.device;
        unsafe {
            for frame in &mut self.frames {
                device.destroy_framebuffer(frame.buffer, None);
                device.destroy_image_view(frame.depth_view, None);
                device.destroy_image_view(frame.view, None);
                frame.depth.destroy(device);
                device.destroy_semaphore(frame.present, None);
            }
            self.swapchain_fn.destroy_swapchain(self.handle, None);
        }
    }
}

struct Frame {
    /// Image view for an entire swapchain image
    view: vk::ImageView,
    /// Depth buffer to use when rendering to this image
    depth: DedicatedImage,
    /// View thereof
    depth_view: vk::ImageView,
    /// Framebuffer referencing `view` and `depth_view`
    buffer: vk::Framebuffer,
    /// Semaphore used to ensure the frame isn't presented until rendering completes
    present: vk::Semaphore,
}

#[derive(Default)]
struct InputState {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    jump: bool,
    clockwise: bool,
    anticlockwise: bool,
    mouse_captured: bool,
}

impl InputState {
    fn clear_motion(&mut self) {
        self.forward = false;
        self.back = false;
        self.left = false;
        self.right = false;
        self.up = false;
        self.down = false;
        self.jump = false;
        self.clockwise = false;
        self.anticlockwise = false;
    }

    fn movement(&self) -> na::Vector3<f32> {
        na::Vector3::new(
            self.right as u8 as f32 - self.left as u8 as f32,
            self.up as u8 as f32 - self.down as u8 as f32,
            self.back as u8 as f32 - self.forward as u8 as f32,
        )
    }

    fn roll(&self) -> f32 {
        self.anticlockwise as u8 as f32 - self.clockwise as u8 as f32
    }
}
