#version 330 core

in float v_alpha;
uniform vec4 u_color;

out vec4 color;

void main()
{
    color = vec4(u_color.rgb, u_color.a * v_alpha);
}
