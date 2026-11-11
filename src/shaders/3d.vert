#version 330 core
uniform mat4 u_model_matrix;
uniform mat4 u_view_matrix;
uniform mat4 u_proj_matrix;

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_uv;

out vec3 v_normal;
out vec2 v_uv;

void main() {
    v_normal = mat3(u_model_matrix) * a_normal;
    v_uv = a_uv.xy;
    gl_Position = u_proj_matrix * u_view_matrix * u_model_matrix * vec4(a_position, 1.0);
}
