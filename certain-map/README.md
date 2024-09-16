# Certain Map
[![Crates.io](https://img.shields.io/crates/v/certain-map.svg)](https://crates.io/crates/certain-map)

> 0.3 is published! It has a new style: "prefilled". This style is more efficient and more flexible. See [migration guide](docs/v2-to-v3-mig.md) for more details.

A typed map that ensures the existence of an item(but it is not a map internally, in fact it is a generated struct).

## What Problem Does It Solve
In Rust, Service abstractions are commonly used for modular structure design, for example [tower-service](https://crates.io/crates/tower-service) or [service-async](https://github.com/ihciah/service-async). Services are layered, and the Request/Response types may vary across different layers. When components across layers have data dependencies, particularly indirect ones, passing all required information by modifying the Request/Response type becomes challenging. If the number of variables to be passed fluctuates, we must redefine a struct to accommodate these changes. This requires implementing conversion functions and data extraction functions for these structs, which can be tedious and can clutter the code. Typically, we avoid this by using HashMap or TypeMap to manage information that needs to be passed across Services.

However, this approach has a significant drawback: we cannot ensure at compile time that the key-value pair required by subsequent components has been set when it is read. This can lead to unnecessary error handling branches in our program or panic in certain scenarios. This crate transforms the struct type when keys are inserted or removed, ensuring the existence of some values at compile-time.

If you need to pass information between multiple stages using a structure, this crate is ideal for you.

It upholds the promise: if it compiles, it works.

## Internal workings(v0.2 version)
> For 0.3 version, see [migration guide](docs/v2-to-v3-mig.md).

```rust
pub type EmptyContext = Context<::certain_map::Vacancy, ::certain_map::Vacancy>;
pub type FullContext =
    Context<::certain_map::Occupied<PeerAddr>, ::certain_map::Occupied<Option<RemoteAddr>>>;
#[derive(Debug, Clone)]
pub struct Context<_CMT_0, _CMT_1> {
    peer_addr: _CMT_0,
    remote_addr: _CMT_1,
}

// `ParamSet for PeerAddr will not compile if it has
// been previously set.
impl<_CMT_0, _CMT_1> ::certain_map::ParamSet<PeerAddr> for Context<_CMT_0, _CMT_1> {
    type Transformed = Context<::certain_map::Occupied<PeerAddr>, _CMT_1>;
    #[inline]
    fn param_set(self, item: PeerAddr) -> Self::Transformed {
        Context {
            peer_addr: ::certain_map::Occupied(item),
            remote_addr: self.remote_addr,
        }
    }
}

// `ParamRef<PeerAddr>` trait bound will not compile for maps
// where it hasn't been set with `ParamSet<PeerAdr>.
impl<_CMT_1> ::certain_map::ParamRef<PeerAddr>
    for Context<::certain_map::Occupied<PeerAddr>, _CMT_1>
{
    #[inline]
    fn param_ref(&self) -> &PeerAddr {
        &self.peer_addr.0
    }
}
```

## Usage(v0.2 version)
> For 0.3 version, see [migration guide](docs/v2-to-v3-mig.md) and [prefilled example](examples/demo_prefilled.rs).

```rust
use certain_map::{certain_map, Param, ParamRef, ParamRemove, ParamSet, ParamTake};

struct UserName(String);

#[derive(Copy, Clone)]
struct UserAge(u8);

certain_map! {
    pub struct MyCertainMap {
        name: UserName,
        #[ensure(Clone)]
        age: UserAge,
    }
}

fn main() {
    let meta = MyCertainMap::new();

    // The following line fails to compile since there's no UserName in the map.
    // log_username(&meta);

    let meta = meta.param_set(UserName("ihciah".to_string()));
    // Now we can get it with certainty.
    log_username(&meta);

    let (meta, removed) = ParamTake::<UserName>::param_take(meta);
    assert_eq!(removed.0, "ihciah");
    // The following line fails to compile since the UserName is removed.
    // log_username(&meta);

    // We can also remove a type no matter if it exist.
    let meta = ParamRemove::<UserName>::param_remove(meta);

    let meta = meta.param_set(UserAge(24));
    // We can get ownership of fields with #[ensure(Clone)]
    log_age(&meta);
}

fn log_username<T: ParamRef<UserName>>(meta: &T) {
    println!("username: {}", meta.param_ref().0);
}

fn log_age<T: Param<UserAge>>(meta: &T) {
    println!("user age: {}", meta.param().0);
}

```