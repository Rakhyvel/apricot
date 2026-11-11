#version 330 core
uniform mat4 u_model_matrix;
uniform mat4 u_view_matrix;
uniform mat4 u_proj_matrix;

layout(location = 0) in vec3 a_position;
layout(location = 2) in vec3 a_uv;

out vec2 v_uv;

void main() {
    v_uv = a_uv.xy;
    gl_Position = u_proj_matrix * u_view_matrix * u_model_matrix * vec4(a_position, 1.0);
}
