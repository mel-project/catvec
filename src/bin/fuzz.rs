use arbitrary::{Arbitrary, Unstructured};
use catvec::CatVec;

#[cfg(fuzzing)]
fn main() {
    use log::LevelFilter;
    let _ = env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .try_init();
    loop {
        honggfuzz::fuzz!(|data: &[u8]| { test_once(&data) });
    }
}

#[derive(Debug, Arbitrary, Clone)]
enum Op {
    Literal(Vec<u8>),
    Append,
}

fn eval(ops: &[Op]) -> Option<CatVec<u8, 8>> {
    let mut stack: Vec<CatVec<u8, 8>> = Vec::new();
    let mut shadow = Vec::new();
    for op in ops {
        match op {
            Op::Literal(v) => {
                shadow.push(v.clone());
                stack.push(v.into())
            }
            Op::Append => {
                let mut x = stack.pop()?;
                let y = stack.pop()?;
                let mut sx = shadow.pop()?;
                let mut sy = shadow.pop()?;
                eprintln!(
                    "popped {} {:?} and {} {:?} of shadow",
                    sx.len(),
                    sx,
                    sy.len(),
                    sy
                );
                x.append(y);
                x.debug_graphviz();
                x.check_invariants();
                stack.push(x);
                sx.append(&mut sy);
                shadow.push(sx);
            }
        }
    }
    stack.pop()
}

fn test_once(data: &[u8]) {
    let data = Vec::<Op>::arbitrary(&mut Unstructured::new(data));
    if let Ok(data) = data {
        eval(&data);
    }
}

#[cfg(not(fuzzing))]
fn main() {}
