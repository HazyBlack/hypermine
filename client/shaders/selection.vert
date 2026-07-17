#version 460

layout(push_constant) uniform PushConstants {
    mat4 chunk_to_clip;
    uvec4 voxel_and_dimension;
    vec4 color;
} pc;

layout(location = 0) out vec4 color_out;

const uvec3 vertices[6][6] = {
    {{0, 0, 0}, {0, 0, 1}, {0, 1, 1}, {0, 1, 1}, {0, 1, 0}, {0, 0, 0}},
    {{1, 0, 0}, {1, 1, 0}, {1, 1, 1}, {1, 1, 1}, {1, 0, 1}, {1, 0, 0}},
    {{0, 0, 0}, {1, 0, 0}, {1, 0, 1}, {1, 0, 1}, {0, 0, 1}, {0, 0, 0}},
    {{0, 1, 0}, {0, 1, 1}, {1, 1, 1}, {1, 1, 1}, {1, 1, 0}, {0, 1, 0}},
    {{0, 0, 0}, {0, 1, 0}, {1, 1, 0}, {1, 1, 0}, {1, 0, 0}, {0, 0, 0}},
    {{0, 0, 1}, {1, 0, 1}, {1, 1, 1}, {1, 1, 1}, {0, 1, 1}, {0, 0, 1}}
};

void main() {
    uint face = gl_VertexIndex / 6;
    uint vertex = gl_VertexIndex % 6;
    vec3 corner = vec3(vertices[face][vertex]);
    // Expand by a small fraction of one voxel to remain visible over the selected surface.
    vec3 expanded = mix(vec3(-0.018), vec3(1.018), corner);
    vec3 chunk_position = (vec3(pc.voxel_and_dimension.xyz) + expanded)
        / float(pc.voxel_and_dimension.w);
    gl_Position = pc.chunk_to_clip * vec4(chunk_position, 1.0);
    color_out = pc.color;
}
