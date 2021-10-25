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
    Insert(usize, u8),
}

fn eval(ops: &[Op]) -> Option<CatVec<u8, 5>> {
    let mut stack: Vec<CatVec<u8, 5>> = Vec::new();
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
                assert_eq!(sx, Vec::from(x.clone()));
                let mut sy = shadow.pop()?;
                assert_eq!(sy, Vec::from(y.clone()));
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
            Op::Insert(i, v) => {
                let mut x = stack.pop()?;
                let mut sx = shadow.pop()?;
                let i = *i % (x.len() + 1);
                eprintln!("insert {} to {:?} pos {}", v, sx, i);
                x.debug_graphviz();
                x.insert(i, *v);
                sx.insert(i, *v);
                eprintln!("------------");
                x.debug_graphviz();
                assert_eq!(sx, Vec::from(x.clone()));
                stack.push(x);
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
