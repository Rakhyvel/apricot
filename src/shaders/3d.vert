#version 330 core
uniform mat4 u_model_matrix;
uniform mat4 u_view_matrix;
uniform mat4 u_proj_matrix;
uniform mat4 light_mvp; // light's proj * view, only used when u_shadows != 0
uniform float u_shadow_texel; // world-space size of one shadow-map texel
uniform vec3 u_sun_dir; // direction towards the light

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_uv;

out vec3 v_normal;
out vec2 v_uv;
out vec4 v_light_space_pos;

void main() {
    vec4 world_pos = u_model_matrix * vec4(a_position, 1.0);
    vec3 n = normalize(mat3(u_model_matrix) * a_normal);
    vec3 l = dot(u_sun_dir, u_sun_dir) > 0.0 ? normalize(u_sun_dir) : normalize(vec3(0.4, 1.0, 0.6));

    // Look up the shadow from slightly above the surface, so it can never shadow itself.
    // 1 texel when facing the light, up to 3 when edge-on, where the self-shadowing error is biggest.
    float slope = 1.0 - clamp(dot(n, l), 0.0, 1.0);
    vec3 offset = n * u_shadow_texel * (1.0 + 2.0 * slope);

    v_normal = n;
    v_uv = a_uv.xy;
    v_light_space_pos = light_mvp * vec4(world_pos.xyz + offset, 1.0);
    gl_Position = u_proj_matrix * u_view_matrix * world_pos;
}
