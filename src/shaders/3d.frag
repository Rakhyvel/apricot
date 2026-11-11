#version 330 core
uniform sampler2D texture0;
uniform sampler2DShadow shadow_map;
uniform int u_shadows; // 0 = no shadow map bound
uniform vec4 u_color;
uniform vec3 u_sun_dir; // direction towards the light, all zeros means "use the default"

in vec3 v_normal;
in vec2 v_uv;
in vec4 v_light_space_pos;
out vec4 color;

// 1.0 = fully lit, 0.0 = fully shadowed. Averages a 3x3 block of texels to soften the edge.
float shadow_visibility() {
    if (u_shadows == 0) return 1.0;
    vec3 p = v_light_space_pos.xyz / v_light_space_pos.w * 0.5 + 0.5;
    if (p.z > 1.0) return 1.0; // past the light's far plane
    vec2 texel = 1.0 / vec2(textureSize(shadow_map, 0));
    float lit = 0.0;
    for (int x = -1; x <= 1; x++)
        for (int y = -1; y <= 1; y++)
            // returns how lit this spot is, 0..1, already blended across neighbouring texels
            lit += texture(shadow_map, vec3(p.xy + vec2(x, y) * texel, p.z));
    float edge = max(abs(p.x - 0.5), abs(p.y - 0.5)) * 2.0; // 0 at the centre, 1 at the box edge
    return mix(lit / 9.0, 1.0, smoothstep(0.8, 1.0, edge));
}

void main() {
    vec3 n = normalize(v_normal);
    vec3 l = dot(u_sun_dir, u_sun_dir) > 0.0 ? normalize(u_sun_dir) : normalize(vec3(0.4, 1.0, 0.6));
    float diffuse = max(dot(n, l), 0.0) * shadow_visibility();
    vec4 base = texture(texture0, v_uv) * u_color;
    color = vec4(base.rgb * (0.2 + 0.8 * diffuse), base.a);
}
