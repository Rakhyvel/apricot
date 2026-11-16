#version 330 core
uniform sampler2D texture0;
uniform vec2 u_sprite_offset; // in UV units
uniform vec2 u_sprite_size; // in UV units
uniform vec4 u_color;

in vec2 v_uv;
out vec4 color;

void main() {
    color = texture(texture0, v_uv * u_sprite_size + u_sprite_offset) * u_color;
}
