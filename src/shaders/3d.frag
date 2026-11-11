#version 330 core
uniform sampler2D texture0;
uniform vec4 u_color;
uniform vec3 u_sun_dir; // direction towards the light, all zeros means "use the default"

in vec3 v_normal;
in vec2 v_uv;
out vec4 color;

void main() {
    vec3 l = dot(u_sun_dir, u_sun_dir) > 0.0 ? normalize(u_sun_dir) : normalize(vec3(0.4, 1.0, 0.6));
    float diffuse = max(dot(normalize(v_normal), l), 0.0);
    vec4 base = texture(texture0, v_uv) * u_color;
    color = vec4(base.rgb * (0.2 + 0.8 * diffuse), base.a);
}
