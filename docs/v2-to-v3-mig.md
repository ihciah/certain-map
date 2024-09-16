# Migrate from v0.2 to v0.3

In v0.2.\*, the `certain_map!` macro generates "unfilled" struct definition. But start from v0.3, it generate "perfilled" struct definition by default.

## Prefilled and Unfilled

### Unfilled Style and its Drawbacks
With "unfilled" style, the genrated code is like:
```rust
struct GeneratedStruct<T1, T2> {
    field1: T1,
    field2: T2,
}
```

And the type is transformed like:
```rust
GeneratedStruct<Vacancy, Vacancy> ->
GeneratedStruct<Occupied<SomeT1>, Vacancy> ->
GeneratedStruct<Occupied<SomeT1>, Occupied<SomeT2>> ->
GeneratedStruct<Vacancy, Occupied<SomeT2>>
```

In this way, the struct's type can promise that certain fields are filled.

However, write(including insert and delete) operations will have to copy all existing fields. This is not efficient.

Also, when it is used across multiple `Service`s, users must pay a lot for stack copying if the struct size is big.

To avoid unnecessary stack copying, I designed a new style: "Prefilled".

### Prefilled Style
With "prefilled" style, the generated code is like:
```rust
struct GeneratedStruct {
    field1: MaybeUninit<SomeT1>,
    field2: MaybeUninit<SomeT2>,
}
struct GeneratedStructState<T1, T2> {
    field1: PhantomData<T1>,
    field2: PhantomData<T2>,
}
struct GeneratedStructHandler<'a, T1, T2> {
    inner: &'a mut GeneratedStruct,
    state: GeneratedStructState<T1, T2>,
}
```

It indeed becomes a little more complex. But it is more efficient and more flexible. Only the `GeneratedStructHandler`'s type is transformed, and this transformation is zero cost.

In this way, users can pass the `GeneratedStructHandler` around without copying the struct itself. Also, when insert or delete a field, there's no need to copy all existing fields.

## Migration
Users can either switch to old "unfilled" style in v0.3(only have to change 1 line), or migrate "prefilled" style(need change more but it has better performance).

### Switch to Unfilled Style
Add `#[style = "unfilled"]` to struct definition like:
```rust
certain_map! {
    #[empty(MyCertainMapEmpty)]
    #[full(MyCertainMapFull)]
    #[style = "unfilled"]
    pub struct MyCertainMap {
        name: UserName,
        #[ensure(Clone)]
        age: UserAge,
    }
}
```

### Use Prefilled Style
1. Due to Rust's limitation, we cannot forward users derive definition to the generated struct now. Now we only support `#[derive(Clone)]`, but later `#[derive(Debug)]` will be supported. So you may remove those unsupported derive.

2. Change the initialization code. You have to change the struct creation and usage to two steps. For example:
```rust
// old
let meta = MyCertainMap::new();
// new
let mut store = MyCertainMap::new();
let meta = store.handler();
```

3. Change the fork code, if you use it. For example:
```rust
// old
let meta_forked = meta.clone();
// new
let (mut store_forked, state_forked) = meta.fork();
let meta_forked = unsafe { state_forked.attach(&mut store_forked) };
```
Note: this requires `#[derive(Clone)]`.
