+++
title = "So I made my own ECS"
date = 2023-07-20
+++

## How it started

I've tried using different ECS libraries in the past, but they just didn't stick with me. I've always liked the idea, but something felt off.

kuviman had a similar experience working with Bevy ECS, and, I think, he has summarized it well [in his devlog](https://kuviman.itch.io/linksider/devlog/520806/i-tried-bevy-for-the-first-time-for-a-game-jam).

ECS just gets too dynamic and hard to debug.

## The idea

So what if we take the idea of separating data, but keep it as static as possible? What if the archetypes were static? What if the queries were checked at compile time? Is that possible?

Turns out, the answer is yes?

So let's dive right into what I've made.

**Content Warning**: advanced Rust ahead.

## Archetypes

This part is basically a generic version of [soa_derive](https://docs.rs/soa_derive/0.13.0/soa_derive/).

Archetypes are defined as normal Rust struct's, with a derive macro:
```rust
#[derive(SplitFields)]
struct Particle {
    position: (f32, f32),
    lifetime: Option<f32>,
}
```

The `SplitFields` derive macro generates a _struct of ~~arrays~~ storages_:
```rust
struct ParticleStructOf<F: StorageFamily> {
    pub position: F::Storage<(f32, f32)>,
    pub lifetime: F::Storage<Option<f32>>,
}
```

`StorageFamily` is essentially ~~a [functor](https://wiki.haskell.org/Functor)~~ a trait for collections that can create new entries and access them by unique id's.

This struct can then magically be used without knowing it's name:
```rust
struct World {
    particles: StructOf<Arena<Particle>>,
}
```

You can read this exactly as it is: `particles` are a struct of arena's for `Particle` (as opposed to an arena of structs `Particle`).

`Arena` here is a collection that implements the `Storage` trait and is of the `ArenaFamily` storage family. You can use any other storage (or even your own), like a `Vec` or a `HashStorage`, but `Arena` should be good for most cases.

## Using `StructOf`

`StructOf` acts as a wrapper around the particular struct's (in our example, the `ParticleStructOf`). It provides the methods to push a new entity, and to get or remove an entity by its id. The push and remove methods take and return, respectively, the user's struct (a `Particle`), so all components must be initialized.
```rust
let id = world.particles.insert(Particle {
    position: (1.0, -0.5),
    lifetime: Some(1.0),
});

let particle: Particle = world.remove(id).unwrap();
```

`StructOf` is actually just a type alias for a more complicated type.
```rust
type StructOf<S: StructOfAble> = <S::Struct as SplitFields<S::Family>>::StructOf;
```

Here you can see the *magic* that allows us to not specify the final `ParticleStructOf`. `StructOfAble` is implemented for all storages for each component type. So, `S` here is `Arena<Particle>`, `S::Struct` is `Particle`, and then `Particle::StructOf` is `ParticleStructOf` (specified by the derive macro).

So, in the end the type expands into `ParticleStructOf<ArenaFamily>`.

## Querying

With the data in-place it is time to have a nice look at it.

Essentially, querying has 3 steps:
  1. Collect references to the storages containing the queried components.
  2. Construct an iterator over the entities.
  3. Combine the queried components into the target view struct.

You can do all steps yourself, but the library does provided shortcuts.

The `query!` macro can be used to query components *immutably* (current limitation) into a tuple or into a struct:
```rust
#[derive(Debug)]
struct PartRef<'a> {
    position: &'a (f32, f32),
    lifetime: &'a f32,
}

// Querying into a struct
for particle in query!(
  world.particles,
  PartRef {
    position,
    lifetime: &lifetime.Get.Some,
  }
) {
    println!("{:?}", particle);
}

// Querying into a tuple
for particle in query!(world.particles, (&position, &lifetime.Get.Some)) {
    println!("{:?}", particle);
}
```

## WTF is `lifetime.Get.Some`

Ever heard of [optics](https://www.schoolofhaskell.com/school/to-infinity-and-beyond/pick-of-the-week/a-little-lens-starter-tutorial)? This is a poor man's version of that.

In simple terms, we access the **lifetime** storage, **get** the component for the entity, and access it only if the variant is **Some** (remember the component type is `Option<f32>`).

You could also specify the position access as `position: &position.Get` or `position: &position` if you wanted to rename a field or be more explicit.

## Mutating data

At the moment, the `query!` macro does not allow mutable access (due to complications with the borrow checker), but the `get!` macro does. It has all the same syntax, just with an additional `id` parameter:
```rust
if let Some(((x, y),)) = get!(world.particles, id, (&mut position)) {
  *x += 1;
  println!("{:?}", (x, y));
}
```

## Nested archetypes

You can also nest one archetype inside another one with a simple macro attribute:
```rust
#[derive(SplitFields)]
struct Explosion {
  radius: f32,
  #[split(nested)]
  particle: Particle,
}

for (radius, position) in query!(world.explosions, (&radius, &particle.position)) {
  // ...
}
```

The resulting structure then has every field, including ones in the nested struct, in its own storage. And you can nest as many struct's as you want.

## Conclusion

That was a rough introduction into what I've been working on lately. If you are interested in the idea, you can read a more complete [example on GitHub](https://github.com/geng-engine/ecs/blob/main/examples/full.rs). I also have a jam game made with this library: [Horns of Combustion](https://github.com/Nertsal/horns-of-combustion/tree/dev).

The project still doesn't have a name, so I'm open to suggestions. It is also not on [crates.io](https://crates.io/) yet, but if anyone is interested and I come up with a name, I will upload it. Let me know :)

