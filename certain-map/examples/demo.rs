use certain_map::{certain_map, ParamRef, ParamRemove, ParamSet};

struct UserName(String);
struct UserCountry(String);

certain_map! {
    pub struct MyCertainMap {
        name: UserName,
        country: UserCountry,
    }
}

fn main() {
    let meta = MyCertainMap::new();

    // The following line compiles fail since there's no UserName in the map.
    // log_username(&meta);

    let meta = meta.set(UserName("ihciah".to_string()));
    // Now we can get it with certainty.
    log_username(&meta);

    let _meta = ParamRemove::<UserName>::remove(meta);
    // The following line compiles fail since the UserName is removed.
    // log_username(&_meta);
}

fn log_username<T: ParamRef<UserName>>(meta: &T) {
    println!("username: {}", meta.param_ref().0);
}
