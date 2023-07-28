+++
title = "So I made my own ECS"
date = 2023-07-20
+++

This article is aimed at people using Rust and interested in ECS.

**Content Warning**: advanced Rust ahead.

If you are only interested in the code, check the [example on GitHub](https://github.com/geng-engine/ecs/blob/main/examples/full.rs).

**TLDR**; ECS is hard to debug, so what if we make entity archetypes static and checked at compile time? We can derive structs and queries using macros and keep user code close to being regular Rust.

## The problem

ECS (Entity Component System) is an architectural pattern widely used in game dev for performance and flexibility reasons. I've tried using different ECS libraries in the past, but they just didn't stick with me. I've always liked the idea, but something felt off.

kuviman had a similar experience working with Bevy ECS, and, I think, he has summarized it well [in his devlog](https://kuviman.itch.io/linksider/devlog/520806/i-tried-bevy-for-the-first-time-for-a-game-jam).

ECS goes against the explicitness and type safety of Rust. With entities being dynamic it practically turns into a dynamically typed language. Additionally, it makes it hard to know which entities get affected by the systems. It is often hard to debug why some specific entity is not behaving in the way you expect, often just because of a missing component.

ECS just gets too dynamic and hard to debug.

## The idea

So what if we take the idea of separating data, but make archetypes static? What if the queries were checked at compile time?

So let's dive right into what I've made.

## Archetypes

This part is basically a generic version of [soa_derive](https://docs.rs/soa_derive/0.13.0/soa_derive/).

In ECS, an archetype is a collection of entities that share the same components. They are used to improve query performance. Usually, entities change their archetypes as you add or remove components.

In our case, archetypes are made static and defined by the user, so they can be checked at compile time.
Archetypes are defined as normal Rust structs, with a derive macro:
```rust
#[derive(SplitFields)]
struct Unit {
    position: (f32, f32),
    health: f32,
    tick: usize,
    damage: Option<f32>,
}
```

The name indicates the underlying meaning: that we just split the fields into their own storages, instead of having them all in the same struct.
So, the `SplitFields` derive macro generates a *struct of ~~arrays~~ storages*:
```rust
struct UnitStructOf<F: StorageFamily> {
    position: F::Storage<(f32, f32)>,
    health: F::Storage<f32>,
    tick: F::Storage<usize>,
    damage: F::Storage<Option<f32>>,
}
```

`StorageFamily` is essentially ~~a [functor](https://wiki.haskell.org/Functor)~~ a trait for collections into which you can insert new items and access items by unique ids.

This struct can then magically be used without knowing its name:
```rust
struct World {
    units: StructOf<Arena<Unit>>,
}
```

You can read this exactly as it is: `units` field is a *struct of arenas for `Unit`* (as opposed to an *arena of structs `Unit`*).

`Arena` here is a collection that implements the `Storage` trait. You can use any other storage (or even your own), like a `Vec` or a `HashStorage`, but `Arena` should be good for most cases.

## Creating and removing entities

`StructOf` acts as a wrapper around a particular struct (in our example, `UnitStructOf`). It provides the methods to insert a new entity, and to get or remove an entity by its id. The push and remove methods take and return, respectively, the user's struct (a `Unit`), so all components must be initialized.
```rust
let id = world.units.insert(Unit {
    pos: (0.0, 0.0),
    health: 10.0,
    tick: 7,
    damage: None,
});

let unit: Unit = world.units.remove(id).unwrap();
```

**Technical note**: `StructOf` is actually just a type alias for a more complicated type.
```rust
type StructOf<S: StructOfAble> = <S::Struct as SplitFields<S::Family>>::StructOf;
```

Here you can see the *magic* that allows us to not specify the final `ParticleStructOf`. `StructOfAble` is implemented for all storages for each component type. So, `S` here is `Arena<Unit>`, `S::Struct` is `Unit`, and then `Unit::StructOf` is `UnitStructOf` (specified by the derive macro).

So, in the end the type expands into `UnitStructOf<ArenaFamily>`.

## Querying

With the data in-place it is time to have a nice look at it.

Essentially, querying has 3 steps:
  1. Collect references to the storages containing the queried components.
  2. Construct an iterator over the entities.
  3. Combine the queried components into the target view struct (or tuple).

You can do all steps yourself, but the library does provided shortcuts.

The `query!` macro can be used to query components *immutably* (current limitation) into a tuple or into a struct.
For example, let's query units that have some damage (not `None`), and also get their position.
```rust
// Querying into a tuple
for unit in query!(world.units, (&position, &damage.Get.Some)) {
    println!("{:?}", unit);
}

// Querying into a struct

// 1. define the struct
#[derive(Debug)]
struct UnitRef<'a> {
    position: &'a (f32, f32),
    damage: &'a f32,
}

// 2. query
for unit in query!(
  world.units,
  UnitRef {
    position,
    damage: &damage.Get.Some,
  }
) {
    println!("{:?}", unit);
}
```

The syntax is mostly identical to normal tuple and struct instantiations with a little change in the field access. I won't go into details about how it is implemented, but you can try expanding the macros and looking at the generated code.

Ok, so...

## WTF is `lifetime.Get.Some`

Ever heard of [optics](https://www.schoolofhaskell.com/school/to-infinity-and-beyond/pick-of-the-week/a-little-lens-starter-tutorial)? This is a poor man's version of that.

In simple terms, we access the **lifetime** storage, **get** the component for the entity, and access it only if the variant is **Some** (remember the component type is `Option<f32>`).

You could also specify the position access as `position: &position.Get` or `position: &position` if you wanted to rename a field or be more explicit.

## Mutating data

At the moment, the `query!` macro does not allow mutable access (due to complications with the borrow checker), but the `get!` macro does. It has all the same syntax, just with an additional `id` parameter. For example, let's reduce health of all units by 5:
```rust
for id in world.units.ids() {
  let (health,) = get!(world.units, id, (&mut health)).unwrap();
  *health -= 5.0;
}
```

## Nested archetypes

You can also nest one archetype inside another one with a simple macro attribute:
```rust
#[derive(SplitFields)]
struct Corpse {
    #[split(nested)]
    unit: Unit,
    time: f32,
}

for (time, position) in query!(world.corpses, (&time, &unit.position)) {
  // ...
}
```

The resulting structure then has every field, including ones in the nested struct, in its own storage. And you can nest as many struct's as you want.

## Extra details

These were the basics of working with the library, but there are more details on how to make use of the features:
- Mutably iterating over different components at once: can easily be checked by the borrow checker since they are just fields in a struct.
- Querying the whole nested storage.
- Combining (chaining) queries over different archetypes.

I won't go over them here, but you can see the code in the [example](https://github.com/geng-engine/ecs/blob/main/examples/full.rs).

## Conclusion

That was a rough introduction into what I've been working on lately. If you like the idea and still want to see more, I also have a jam game made with this library: [Horns of Combustion](https://github.com/Nertsal/horns-of-combustion/tree/dev).

The project still doesn't have a name, so I'm open to suggestions. It is also not on [crates.io](https://crates.io/) yet, but if anyone is interested and I come up with a name, I will upload it. Let me know :)

