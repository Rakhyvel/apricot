/// Every scene must have a table of function pointers that match this structure.
pub const Scene_VTable = struct {
    deinit: *const fn (self: *void) void,
    update: *const fn (self: *void) void,
    render: *const fn (self: *void) void,
};

/// A Scene Object is a pointer to the object data itself, and a pointer to the corresponding vtable.
pub const Scene_Object = struct {
    self: *void,
    vtable: *const Scene_VTable,
};
