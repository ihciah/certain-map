// Copyright 2024 ihciah. All Rights Reserved.

//! This demo is used to show how to use certain_map with `Services`(no matter tower Service or
//! service-async Service).
//! To pass information across service layers, and make it able to decouple the Context concrete
//! type, we can use certain_map with param crate.

use std::{convert::Infallible, future::Future, marker::PhantomData, ops::Add};

use certain_map::{Attach, Fork, Handler};
use certain_map_macros::certain_map;
use param::{ParamRef, ParamSet};

// Copied from https://github.com/ihciah/service-async/blob/master/service-async/src/lib.rs
// Copy to avoid adding dev-dependency.
trait Service<Request> {
    type Response;
    type Error;
    fn call(&self, req: Request) -> impl Future<Output = Result<Self::Response, Self::Error>>;
}

#[derive(Clone)]
pub struct RawBeforeAdd(u8);

#[derive(Clone)]
pub struct RawBeforeMul(u8);

certain_map! {
    #[empty(MyCertainMapEmpty)]
    #[full(MyCertainMapFull)]
    #[derive(Clone)]
    pub struct MyCertainMap {
        raw_before_add: RawBeforeAdd,
        raw_before_mul: RawBeforeMul,
    }
}

// Define a service that adds 1 to the input number.
struct Add1<T>(T);

impl<T, CX> Service<(u8, CX)> for Add1<T>
where
    T: Service<(u8, CX::Transformed)>,
    CX: ParamSet<RawBeforeAdd>,
{
    type Response = T::Response;
    type Error = T::Error;

    fn call(
        &self,
        (num, cx): (u8, CX),
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> {
        self.0.call((num + 1, cx.param_set(RawBeforeAdd(num))))
    }
}

// Define a service that multiplies the input number by 2.
struct Mul2<T>(T);

impl<T, CX> Service<(u8, CX)> for Mul2<T>
where
    T: Service<(u8, CX::Transformed)>,
    CX: ParamSet<RawBeforeMul>,
{
    type Response = T::Response;
    type Error = T::Error;

    fn call(
        &self,
        (num, cx): (u8, CX),
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> {
        self.0.call((num * 2, cx.param_set(RawBeforeMul(num))))
    }
}

// Define a service that prints the context and return the input.
struct Identical;

impl<CX> Service<(u8, CX)> for Identical
where
    CX: ParamRef<RawBeforeAdd> + ParamRef<RawBeforeMul>,
{
    type Response = u8;
    type Error = Infallible;

    async fn call(&self, (num, cx): (u8, CX)) -> Result<Self::Response, Self::Error> {
        println!(
            "num before add: {}",
            ParamRef::<RawBeforeAdd>::param_ref(&cx).0
        );
        println!(
            "num before mul: {}",
            ParamRef::<RawBeforeMul>::param_ref(&cx).0
        );
        println!("num: {num}");
        Ok(num)
    }
}

// A service that create a context and call the inner service.
struct CXSvc<CXStore, T> {
    inner: T,
    cx: PhantomData<CXStore>,
}

impl<CXStore, T> CXSvc<CXStore, T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            cx: PhantomData,
        }
    }
}

impl<CXStore, T, R, RESP, ERR> Service<R> for CXSvc<CXStore, T>
where
    CXStore: Handler + Default + 'static,
    for<'a> T: Service<(R, CXStore::Hdr<'a>), Response = RESP, Error = ERR>,
{
    type Response = RESP;
    type Error = ERR;

    async fn call(&self, num: R) -> Result<Self::Response, Self::Error> {
        let mut store = CXStore::default();
        let hdr = store.handler();
        self.inner.call((num, hdr)).await
    }
}

// A service that call inner twice and return the sum.
// This is to show how to fork context.
struct DupSvc<T>(T);

impl<T, R, CXIn, CXStore, CXState, Resp, Err> Service<(R, CXIn)> for DupSvc<T>
where
    R: Copy,
    Resp: Add<Output = Resp>,
    CXIn: Fork<Store = CXStore, State = CXState>,
    CXStore: 'static,
    for<'a> CXState: Attach<CXStore>,
    for<'a> T: Service<(R, <CXState as Attach<CXStore>>::Hdr<'a>), Response = Resp, Error = Err>,
{
    type Response = Resp;
    type Error = Err;

    async fn call(&self, (req, ctx): (R, CXIn)) -> Result<Self::Response, Self::Error> {
        // fork ctx
        let (mut store, state) = ctx.fork();
        let forked_ctx = unsafe { state.attach(&mut store) };
        let r1 = self.0.call((req, forked_ctx)).await?;

        // fork ctx
        let (mut store, state) = ctx.fork();
        let forked_ctx = unsafe { state.attach(&mut store) };
        let r2 = self.0.call((req, forked_ctx)).await?;

        Ok(r1 + r2)
    }
}

#[tokio::main]
async fn main() {
    // Create service and initialize store, then call with the handler.
    // (2 + 1) * 2 = 6 is expected.
    let svc = Add1(Mul2(Identical));
    let mut store = MyCertainMap::new();
    assert_eq!(svc.call((2, store.handler())).await.unwrap(), 6);

    // You can even create a service to initialize store and pass the handler.
    let svc = CXSvc::<MyCertainMap, _>::new(svc);
    assert_eq!(svc.call(2).await.unwrap(), 6);

    // To show how to fork ctx.
    let svc = CXSvc::<MyCertainMap, _>::new(DupSvc(Add1(Mul2(Identical))));
    // It is expected to print 2 times.
    assert_eq!(svc.call(2).await.unwrap(), 12);
}
