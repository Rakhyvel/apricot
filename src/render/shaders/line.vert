#version 330 core

layout(location = 0) in vec3 a_pos;

uniform mat4 u_proj_matrix;
uniform mat4 u_view_matrix;
uniform mat4 u_model_matrix;
uniform float u_seam; // 0.0 to 1.0, fraction along the orbit where the craft is
uniform int u_num_vertices;
uniform int u_fade;

out float v_alpha;

void main() {
    if (u_fade != 0)
    {
        // gl_VertexID gives us which vertex we are, 0 to u_num_vertices
        float t = float(gl_VertexID) / float(u_num_vertices);
        float dist = mod(t - u_seam + 1.0, 1.0); // circular distance from seam
        v_alpha = 1.0 - pow(0.894 * (dist - 1.0), 2.0);
    }
    else
    {
        v_alpha = 1.0;
    }

    gl_Position = (u_proj_matrix * u_view_matrix * u_model_matrix * vec4(a_pos, 1.0));
}
