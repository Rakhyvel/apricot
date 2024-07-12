const std = @import("std");
const SDL = @import("sdl2");
const gl = @import("zgl");
const glm = @import("zlm");

pub fn texture_from_surface(surf: SDL.Surface) gl.Texture {
    const retval = gl.genTexture();
    gl.bindTexture(retval, .@"2d");

    const width = surf.ptr.w;
    const height = surf.ptr.h;
    gl.textureImage2D(
        .@"2d",
        0,
        .rgba,
        @intCast(width),
        @intCast(height),
        gl.PixelFormat.bgra,
        .unsigned_byte,
        @ptrCast(surf.ptr.pixels.?),
    );
    retval.parameter(.wrap_s, .clamp_to_edge);
    retval.parameter(.wrap_t, .clamp_to_edge);
    retval.parameter(.min_filter, .nearest);
    retval.parameter(.mag_filter, .nearest);
    gl.bindTexture(.invalid, .@"2d");
    return retval;
}

pub fn create_program(vertex_source: []const u8, fragment_source: []const u8) !gl.Program {
    const alloc = std.heap.page_allocator;

    // Create the program
    const program = gl.createProgram();
    errdefer program.delete();

    // Create vertex shader
    const vShader = gl.createShader(.vertex);
    errdefer vShader.delete();
    vShader.source(1, &.{vertex_source});
    gl.compileShader(vShader);
    gl.attachShader(program, vShader);
    const v_shader_log = try vShader.getCompileLog(alloc);
    defer alloc.free(v_shader_log);
    if (v_shader_log.len > 0) {
        std.debug.print("vertex shader error: {s}", .{v_shader_log});
        return error.OpenGLError;
    }

    // Create fragment shader
    const fShader = gl.createShader(.fragment);
    errdefer fShader.delete();
    fShader.source(1, &.{fragment_source});
    gl.compileShader(fShader);
    gl.attachShader(program, fShader);
    const f_shader_log = try fShader.getCompileLog(alloc);
    defer alloc.free(f_shader_log);
    if (f_shader_log.len > 0) {
        std.debug.print("fragment shader error: {s}", .{f_shader_log});
        return error.OpenGLError;
    }

    // Link the program's pipeline up
    gl.linkProgram(program);
    const program_log = try program.getCompileLog(alloc);
    defer alloc.free(program_log);
    if (program_log.len > 0) {
        std.debug.print("program error: {s}", .{program_log});
        return error.OpenGLError;
    }

    return program;
}

pub const Mesh = struct {
    vao: gl.VertexArray,
    vbo: gl.Buffer,
    ebo: gl.Buffer,
    n: usize,

    pub fn init() Mesh {
        return .{
            .vao = .invalid,
            .vbo = .invalid,
            .ebo = .invalid,
            .n = 0,
        };
    }

    pub fn from_data(vertices: []align(1) const f32, indices: []align(1) const gl.UInt) Mesh {
        var retval = Mesh.init();

        // Create and bind VAO, VBO, and EBO
        retval.vao = gl.createVertexArray();
        gl.bindVertexArray(retval.vao);

        retval.vbo = gl.createBuffer();
        gl.bindBuffer(retval.vbo, .array_buffer);
        gl.bufferData(.array_buffer, f32, vertices, .static_draw);

        retval.ebo = gl.createBuffer();
        gl.bindBuffer(retval.ebo, .element_array_buffer);
        gl.bufferData(.element_array_buffer, gl.UInt, indices, .static_draw);

        retval.n = indices.len;

        // Set vertex attribute pointers
        gl.vertexAttribPointer(
            0,
            3,
            .float,
            false,
            3 * @sizeOf(f32),
            0,
        );
        gl.enableVertexAttribArray(0);

        // Set UV attribute pointers
        gl.vertexAttribPointer(
            1,
            2,
            .float,
            false,
            2 * @sizeOf(f32),
            12 * @sizeOf(f32),
        );
        gl.enableVertexAttribArray(1);

        // Unbind VAO
        gl.bindVertexArray(.invalid);

        return retval;
    }

    pub fn draw(self: *const Mesh, model: glm.Mat4, view: glm.Mat4, proj: glm.Mat4, program: gl.Program, texture: gl.Texture) void {
        gl.useProgram(program);

        const u_model_idx = gl.getUniformLocation(program, "u_model");
        const u_view_idx = gl.getUniformLocation(program, "u_view");
        const u_proj_idx = gl.getUniformLocation(program, "u_proj");

        gl.uniformMatrix4fv(u_model_idx, false, &.{model.fields});
        gl.uniformMatrix4fv(u_view_idx, false, &.{view.fields});
        gl.uniformMatrix4fv(u_proj_idx, false, &.{proj.fields});

        gl.bindTexture(texture, .@"2d");
        gl.bindVertexArray(self.vao);
        gl.drawElements(.triangles, self.n, .unsigned_int, 0);
    }
};

pub const vertexShaderSource_2d: []const u8 =
    \\#version 450 core
    \\layout(location = 0) in vec3 l_pos;
    \\layout(location = 1) in vec2 l_tex_coord;
    \\out vec2 tex_coord;
    \\uniform mat4 u_model;
    \\uniform mat4 u_view;
    \\uniform mat4 u_proj;
    \\void main() {
    \\    gl_Position = u_proj * u_view * u_model * vec4(l_pos, 1);
    \\    tex_coord = l_tex_coord;
    \\}
;

pub const fragmentShaderSource_3d: []const u8 =
    \\#version 450 core
    \\in vec2 tex_coord;
    \\out vec4 color;
    \\uniform sampler2D u_texture1;
    \\void main() {
    \\    color = texture(u_texture1, tex_coord);
    \\}
;

pub const quad_vertices = [_]f32{
    0.0, 0.0, 0.0, // 0 left top |^
    1.0, 0.0, 0.0, // 1 right top ^|
    0.0, 1.0, 0.0, // 2 left bottom |_
    1.0, 1.0, 0.0, // 3 right bottom _|
    0.0, 0.0, // UV
    1.0, 0.0,
    0.0, 1.0,
    1.0, 1.0,
};

pub const quad_uv = [_]f32{
    0.0, 0.0, // UV
    1.0, 0.0,
    0.0, 1.0,
    1.0, 1.0,
};

pub const quad_indices = [_]gl.UInt{
    0, 3, 1, // first triangle    \|
    0, 2, 3, // second triangle |\
};
