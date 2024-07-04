const std = @import("std");

/// A unique identifier for an entity
pub const Entity_Id: type = struct {
    idx: Entity_Index,
    vers: Entity_Version,

    pub const INVALID_ENTITY_ID: Entity_Id = Entity_Id.init(INVALID_ENTITY_INDEX, 0);

    pub inline fn init(idx: Entity_Index, vers: Entity_Version) Entity_Id {
        return .{ .idx = idx, .vers = vers };
    }

    pub inline fn index(self: Entity_Id) Entity_Index {
        return self.idx;
    }

    pub inline fn version(self: Entity_Id) Entity_Version {
        return self.vers;
    }
};

/// The index of an entity in a scene's component and bitmask arrays
pub const Entity_Index: type = u16;
const INVALID_ENTITY_INDEX: Entity_Index = ~@as(Entity_Index, @intCast(0x0));

/// The version of an entity given it's index.
pub const Entity_Version: type = u16;

/// A random universal key that corresponds to a component.
const Type_Uid: type = usize;
const INVALID_COMPONENT_KEY: Type_Uid = ~@as(Type_Uid, @intCast(0x0));

/// Actual ID of the component as mapped in the world
const Component_Id: type = u6;

/// Bitmask of Component_Id's that can be used to specify a collection of components
const Component_Mask: type = usize;
const MAX_COMPONENTS: usize = 64;

const Entity = struct {
    /// The unique identifier of this entity
    id: Entity_Id,
    /// The components this entity has
    mask: Component_Mask,
};

/// A struct that allows deferred addition of entities into a world with components. This is useful in the middle of a
/// system, where adding entities has the potential to invalidate component data pointers.
const Lazy_Entity_Builder = struct {
    world: *World,
    id: Entity_Id,
    pool: Component_Data_Pool,
    components: std.ArrayList(Component_Pair),
    const Component_Pair = struct { component_id: Component_Id, data_idx: usize };

    fn init(self: *Lazy_Entity_Builder, world: *World, id: Entity_Id) !void {
        self.world = world;
        self.id = id;
        self.pool = try Component_Data_Pool.init(1, world.alloc);
        self.components = std.ArrayList(Component_Pair).init(world.alloc);

        try self.world.lazy_entity_builders.append(self);
    }

    fn deinit(self: *Lazy_Entity_Builder) void {
        self.pool.deinit();
        self.components.deinit();
        self.world.alloc.destroy(self);
    }

    /// Specifies component data that should be associated with the entity when it is created. Returns the pointer to
    /// the builder, for a nice pipeline pattern
    pub fn with(self: *Lazy_Entity_Builder, data: anytype) *Lazy_Entity_Builder {
        const component_id = self.world.get_component_id(@TypeOf(data));
        var mut_data = data;
        const idx = self.pool.data.items.len;
        const size = @sizeOf(@TypeOf(data));
        self.components.append(.{ .component_id = component_id, .data_idx = idx }) catch @panic("memory error!");
        self.pool.assert_size(idx + size) catch @panic("memory error!");
        @memcpy(
            self.pool.data.items[idx .. idx + size],
            @as([*]u8, @ptrCast(&mut_data))[0..size],
        );
        return self;
    }

    /// Returns the ID of the entity being constructed
    pub fn entity_id(self: *Lazy_Entity_Builder) Entity_Id {
        return self.id;
    }
};

/// Provides:
/// - Contiguous storage
/// - Fast O(1) access of data pointer given Entity_Id
/// - Unlikely to invalidate pointers on resize (impossible when using lazy entity builders)
/// - Not a fixed buffer size
const Component_Data_Pool = struct {
    data: std.ArrayList(u8),
    stride_length: usize,

    // The address of this is returned whenever the stride-length is 0
    var empty_byte: u8 = 0;

    pub fn init(stride_length: usize, alloc: std.mem.Allocator) !Component_Data_Pool {
        return .{
            .data = try std.ArrayList(u8).initCapacity(alloc, 64),
            .stride_length = stride_length,
        };
    }

    pub fn deinit(self: *Component_Data_Pool) void {
        self.data.deinit();
    }

    pub inline fn get_data(self: *Component_Data_Pool, index: usize) *u8 {
        if (self.stride_length == 0) {
            return &empty_byte;
        }
        return &self.data.items[index * self.stride_length];
    }

    pub fn assert_size(self: *Component_Data_Pool, element_size: usize) !void {
        if (self.stride_length == 0) {
            return;
        }
        const byte_size = element_size * self.stride_length;
        if (self.data.items.len >= byte_size) {
            return;
        }

        const new_capacity = try std.math.ceilPowerOfTwo(usize, byte_size);
        // This has the chance to invalidate pointers, unless
        try self.data.appendNTimes(0, new_capacity - self.data.items.len);
    }

    pub fn memcpy(self: *Component_Data_Pool, index: usize, data: [*]u8) void {
        if (self.stride_length == 0) {
            return;
        }
        @memcpy(
            self.data.items[index * self.stride_length .. (index + 1) * self.stride_length],
            data[0..self.stride_length],
        );
    }
};

pub const World = struct {
    /// Lookup table used to map component keys to their actual index in this particular scene
    lookup_table: [MAX_COMPONENTS]Type_Uid,
    /// Array of arraylists of bytes, represents the actual data for each component
    components: [MAX_COMPONENTS]?Component_Data_Pool,
    /// Data used to hold resources
    resource_map: std.AutoArrayHashMap(Type_Uid, *void),
    /// List of all entities in the scene
    entities: std.ArrayList(Entity),
    /// List of entities marked to be purged
    purged_entities: std.ArrayList(Entity_Index),
    /// List of newly free indices that can be overwritten
    free_indices: std.ArrayList(Entity_Index),
    /// List of lazy entity builders to maintain
    lazy_entity_builders: std.ArrayList(*Lazy_Entity_Builder),
    /// Number of entities in the scene. Not necessarily the length of the entities list
    num_entities: usize,
    /// Number of components registered to this scene
    num_components: usize,
    /// Allocator for this world
    alloc: std.mem.Allocator,

    pub fn init(alloc: std.mem.Allocator) World {
        return .{
            .lookup_table = [_]Type_Uid{INVALID_COMPONENT_KEY} ** MAX_COMPONENTS,
            .components = [_]?Component_Data_Pool{null} ** MAX_COMPONENTS,
            .resource_map = std.AutoArrayHashMap(Type_Uid, *void).init(alloc),
            .entities = std.ArrayList(Entity).init(alloc),
            .purged_entities = std.ArrayList(Entity_Index).init(alloc),
            .free_indices = std.ArrayList(Entity_Index).init(alloc),
            .lazy_entity_builders = std.ArrayList(*Lazy_Entity_Builder).init(alloc),
            .num_entities = 0,
            .num_components = 0,
            .alloc = alloc,
        };
    }

    pub fn deinit(self: *World) void {
        self.entities.deinit();
        self.purged_entities.deinit();
        self.free_indices.deinit();
        for (0..self.num_components) |i| {
            if (self.components[i]) |*pool| {
                pool.deinit();
            }
        }
        for (self.lazy_entity_builders.items) |lazy_builder| {
            lazy_builder.deinit();
        }
        self.lazy_entity_builders.deinit();
        self.resource_map.deinit();
    }

    pub fn register_component(self: *World, comptime C: type) !void {
        const key = type_uid(C);
        self.lookup_table[self.num_components] = key;
        self.components[self.num_components] = try Component_Data_Pool.init(@sizeOf(C), self.alloc);
        self.num_components += 1;
    }

    pub fn register_resource(self: *World, resource: anytype) !void {
        const Resource_Type = @TypeOf(resource);
        if (@typeInfo(Resource_Type) != .Pointer) {
            std.debug.panic("Resource type is not a pointer", .{});
        }
        const resource_type_uid = type_uid(@typeInfo(Resource_Type).Pointer.child);
        if (self.resource_map.get(resource_type_uid)) |_| {
            std.debug.panic("The resource type `{}` is already registered in this world", .{@typeInfo(Resource_Type).Pointer.child});
        }
        try self.resource_map.put(resource_type_uid, @ptrCast(resource));
    }

    pub fn get_resource(self: *World, comptime R: type) *R {
        const resource_type_uid = type_uid(R);
        return @alignCast(@ptrCast(self.resource_map.get(resource_type_uid).?));
    }

    fn new_entity(self: *World) !Entity_Id {
        if (self.free_indices.items.len != 0) {
            const index = self.free_indices.pop();
            var entity = &self.entities.items[index];
            entity.id = Entity_Id.init(index, entity.id.version() + 1);
            entity.mask = 0;
            self.num_entities += 1;
            return entity.id;
        } else {
            const index: Entity_Index = @intCast(self.entities.items.len);
            const entity = Entity{ .id = Entity_Id.init(index, 0), .mask = 0 };
            try self.entities.append(entity);
            self.num_entities += 1;
            for (0..self.num_components) |i| {
                if (self.components[i]) |*pool| {
                    try pool.assert_size(index + 1);
                }
            }
            return entity.id;
        }
    }

    /// Constructs a Lazy Entity Builder struct, which can be used to defer adding new entities into the world until
    /// `World.maintain` is called.
    pub fn create_entity(self: *World) *Lazy_Entity_Builder {
        const id = self.new_entity() catch @panic("memory error!");
        const retval = self.alloc.create(Lazy_Entity_Builder) catch @panic("memory error!");
        retval.init(self, id) catch @panic("memory error!");
        return retval;
    }

    pub fn assign_component(self: *World, comptime C: type, id: Entity_Id, data: ?C) void {
        const component_id = self.get_component_id(C);
        std.debug.assert(id.index() != INVALID_ENTITY_INDEX);
        std.debug.assert(self.entities.items[id.index()].id.version() == id.version());
        std.debug.assert(self.components[component_id] != null);
        self.get_entity(id).mask |= create_component_mask(component_id);
        if (data) |some_data| {
            var mut_data: C = some_data;
            self.components[component_id].?.memcpy(id.index(), @ptrCast(&mut_data));
        }
    }

    pub fn unassign_component(self: *World, comptime C: type, id: Entity_Id) void {
        const component_id = self.get_component_id(C);
        std.debug.assert(id.index() != INVALID_ENTITY_INDEX);
        std.debug.assert(self.entities.items[id.index()].id.version() == id.version());
        self.get_entity(id).mask &= ~create_component_mask(component_id);
    }

    pub fn mark_purged(self: *World, id: Entity_Id) !void {
        const index = id.index();
        std.debug.assert(id.index() != INVALID_ENTITY_INDEX);
        std.debug.assert(self.entities.items[id.index()].id.version() == id.version());
        try self.purged_entities.append(index);
    }

    pub fn maintain(self: *World) !void {
        while (self.purged_entities.items.len > 0) {
            const index = self.purged_entities.pop();
            var purged_entity = &self.entities.items[index];
            purged_entity.id = Entity_Id.init(INVALID_ENTITY_INDEX, purged_entity.id.version());
            purged_entity.mask = 0;
            self.num_entities -= 1;
            // Must gaurd against duplicates, otherwise allocated entity would be marked "free"
            var contains = false;
            for (0..self.free_indices.items.len) |i| {
                if (self.free_indices.items[i] == index) {
                    contains = true;
                    break;
                }
            }
            if (!contains) {
                try self.free_indices.append(index);
            }
        }

        while (self.lazy_entity_builders.items.len > 0) {
            const lazy_builder = self.lazy_entity_builders.pop();
            for (lazy_builder.components.items) |comp| {
                const component_id = comp.component_id;
                const data_idx = comp.data_idx;
                std.debug.assert(self.components[component_id] != null);
                self.get_entity(lazy_builder.id).mask |= create_component_mask(component_id);
                self.components[component_id].?.memcpy(lazy_builder.id.index(), @ptrCast(lazy_builder.pool.get_data(data_idx)));
            }
            lazy_builder.deinit();
        }
    }

    pub fn iter(self: *World, comptime Components: type) Iter(Components) {
        return Iter(Components).init(self);
    }

    fn Iter(comptime Components: type) type {
        return struct {
            world: *World,
            index: Entity_Index,
            mask: Component_Mask,

            fn init(world: *World) @This() {
                return .{
                    .world = world,
                    .index = 0,
                    .mask = world.create_mask(Components),
                };
            }

            pub inline fn next(self: *@This()) ?Components {
                while (self.index < self.world.entities.items.len) : (self.index += 1) {
                    const entity = self.world.entities.items[self.index];
                    // Below is the *only* branch in the entire
                    if (entity.id.index() != INVALID_ENTITY_INDEX and (entity.mask & self.mask) == self.mask) {
                        self.index += 1;
                        return self.create_struct(entity.id);
                    }
                } else {
                    return null;
                }
            }

            inline fn create_struct(self: *@This(), id: Entity_Id) Components {
                var result: Components = undefined;

                inline for (@typeInfo(Components).Struct.fields) |field| {
                    const Field_Type = field.type;
                    var num_ids_requested: usize = 0;
                    if (Field_Type == Entity_Id) {
                        // User has requested the id in the struct, fill the field with the ID
                        if (num_ids_requested >= 1) {
                            @panic("did you really want that entity id more than once?");
                        }
                        num_ids_requested += 1;
                        @field(result, field.name) = id;
                    } else if (@typeInfo(Field_Type) == .Pointer) {
                        // User has requested a component, fill the field with the pointer to the entity's component data
                        const component_ptr = self.world.get_component(@typeInfo(Field_Type).Pointer.child, id);
                        @field(result, field.name) = component_ptr;
                    } else {
                        std.debug.panic("Field type `{}` is not a pointer nor entity ID\n", .{Field_Type});
                    }
                }
                return result;
            }
        };
    }

    pub fn get_component(self: *World, comptime C: type, id: Entity_Id) *C {
        const component_id = self.get_component_id(C);
        std.debug.assert(id.index() != INVALID_ENTITY_INDEX);
        std.debug.assert(self.entities.items[id.index()].id.version() == id.version());
        std.debug.assert(self.entity_has_all_components(struct { c: C }, id));
        std.debug.assert(self.components[component_id] != null);
        const raw_data = self.components[component_id].?.get_data(id.index());
        return @alignCast(@ptrCast(raw_data));
    }

    /// Checks that an Entity_Id refers to a valid and current entity
    pub fn entity_is_valid(self: *World, id: Entity_Id) bool {
        const entity = self.entities.items[id.index()];
        return entity.mask != 0 and entity.id == id;
    }

    pub fn entity_has_all_components(self: *World, comptime Components: type, id: Entity_Id) bool {
        const index = id.index();
        const entity = self.entities.items[index];
        const mask = self.create_mask(Components);
        std.debug.assert(id.index() != INVALID_ENTITY_INDEX);
        std.debug.assert(self.entities.items[id.index()].id.version() == id.version());
        return (entity.mask & mask) == mask;
    }

    pub fn entity_has_any_components(self: *World, comptime Components: type, id: Entity_Id) bool {
        const index = id.index();
        const entity = self.entities.items[index];
        const mask = self.create_mask(Components);
        std.debug.assert(id.index() != INVALID_ENTITY_INDEX);
        std.debug.assert(self.entities.items[id.index()].id.version() == id.version());
        return (entity.mask & mask) != 0;
    }

    inline fn get_component_id(self: *World, comptime C: type) Component_Id {
        const key = type_uid(C);
        inline for (0..MAX_COMPONENTS) |i| {
            if (self.lookup_table[i] == key) {
                return @intCast(i);
            }
        }
        std.debug.panic("Component type `{}` not registered in the world!", .{C});
    }

    fn get_entity(self: *World, id: Entity_Id) *Entity {
        return &self.entities.items[id.index()];
    }

    fn create_mask(self: *World, comptime Components: type) Component_Mask {
        var retval: Component_Mask = 0;
        inline for (@typeInfo(Components).Struct.fields) |field| {
            const Field_Type = field.type;
            if (Field_Type == Entity_Id) {
                continue;
            }
            const mask = if (@typeInfo(Field_Type) == .Pointer)
                create_component_mask(self.get_component_id(@typeInfo(Field_Type).Pointer.child))
            else
                create_component_mask(self.get_component_id(Field_Type));
            retval |= mask;
        }
        return retval;
    }
};

/// Returns a unique 64-bit identifier for any type
fn type_uid(comptime T: type) Type_Uid {
    const H = struct {
        var byte: u8 = 0;
        var hmm: T = undefined;
    };
    return @intFromPtr(&H.byte);
}

fn create_component_mask(id: Component_Id) Component_Mask {
    return @as(Component_Mask, @intCast(1)) << id;
}
