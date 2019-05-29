// ordered container
// Normally, user will only care about the first several options. So we only keep several of them
// in order. Other items are kept unordered and are sorted on demand.

use std::cmp::Ordering;
use rayon::prelude::*;

pub type CompareFunction<T> = Box<Fn(&T, &T) -> Ordering + Send + Sync>;
const ORDERED_SIZE: usize = 300;

pub struct OrderedVec<T: Send> {
    vec: Vec<T>,
    compare: CompareFunction<T>,
}

impl<T: Send> OrderedVec<T> {
    pub fn new(compare: CompareFunction<T>) -> Self {
        OrderedVec {
            vec: Vec::with_capacity(ORDERED_SIZE),
            compare,
        }
    }

    pub fn append_ordered(&mut self, mut items: Vec<T>) {
        // 1. sort the new items
        items.par_sort_unstable_by(self.compare.as_ref());

        // 2. merge the new items with existing array
        let mid = self.vec.len();
        self.vec.append(&mut items);
        unsafe {
            let compare_fn = self.compare.as_mut();
            merge(&mut self.vec, mid, items.as_mut_ptr(), &mut |a, b| compare_fn(a, b) == Ordering::Less);
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.vec.get(index)
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn clear(&mut self) {
        self.vec.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    pub fn iter<'a>(&'a self) -> Box<Iterator<Item = &T> + 'a> {
        Box::new(self.vec.iter())
    }
}

use std::ptr;
use std::mem;

/// Copied from std::slice
/// Merges non-decreasing runs `v[..mid]` and `v[mid..]` using `buf` as temporary storage, and
/// stores the result into `v[..]`.
///
/// # Safety
///
/// The two slices must be non-empty and `mid` must be in bounds. Buffer `buf` must be long enough
/// to hold a copy of the shorter slice. Also, `T` must not be a zero-sized type.
unsafe fn merge<T, F>(v: &mut [T], mid: usize, buf: *mut T, is_less: &mut F)
    where F: FnMut(&T, &T) -> bool
{
    let len = v.len();
    let v = v.as_mut_ptr();
    let v_mid = v.add(mid);
    let v_end = v.add(len);

    // The merge process first copies the shorter run into `buf`. Then it traces the newly copied
    // run and the longer run forwards (or backwards), comparing their next unconsumed elements and
    // copying the lesser (or greater) one into `v`.
    //
    // As soon as the shorter run is fully consumed, the process is done. If the longer run gets
    // consumed first, then we must copy whatever is left of the shorter run into the remaining
    // hole in `v`.
    //
    // Intermediate state of the process is always tracked by `hole`, which serves two purposes:
    // 1. Protects integrity of `v` from panics in `is_less`.
    // 2. Fills the remaining hole in `v` if the longer run gets consumed first.
    //
    // Panic safety:
    //
    // If `is_less` panics at any point during the process, `hole` will get dropped and fill the
    // hole in `v` with the unconsumed range in `buf`, thus ensuring that `v` still holds every
    // object it initially held exactly once.
    let mut hole;

    if mid <= len - mid {
        // The left run is shorter.
        ptr::copy_nonoverlapping(v, buf, mid);
        hole = MergeHole {
            start: buf,
            end: buf.add(mid),
            dest: v,
        };

        // Initially, these pointers point to the beginnings of their arrays.
        let left = &mut hole.start;
        let mut right = v_mid;
        let out = &mut hole.dest;

        while *left < hole.end && right < v_end {
            // Consume the lesser side.
            // If equal, prefer the left run to maintain stability.
            let to_copy = if is_less(&*right, &**left) {
                get_and_increment(&mut right)
            } else {
                get_and_increment(left)
            };
            ptr::copy_nonoverlapping(to_copy, get_and_increment(out), 1);
        }
    } else {
        // The right run is shorter.
        ptr::copy_nonoverlapping(v_mid, buf, len - mid);
        hole = MergeHole {
            start: buf,
            end: buf.add(len - mid),
            dest: v_mid,
        };

        // Initially, these pointers point past the ends of their arrays.
        let left = &mut hole.dest;
        let right = &mut hole.end;
        let mut out = v_end;

        while v < *left && buf < *right {
            // Consume the greater side.
            // If equal, prefer the right run to maintain stability.
            let to_copy = if is_less(&*right.offset(-1), &*left.offset(-1)) {
                decrement_and_get(left)
            } else {
                decrement_and_get(right)
            };
            ptr::copy_nonoverlapping(to_copy, decrement_and_get(&mut out), 1);
        }
    }
    // Finally, `hole` gets dropped. If the shorter run was not fully consumed, whatever remains of
    // it will now be copied into the hole in `v`.

    unsafe fn get_and_increment<T>(ptr: &mut *mut T) -> *mut T {
        let old = *ptr;
        *ptr = ptr.offset(1);
        old
    }

    unsafe fn decrement_and_get<T>(ptr: &mut *mut T) -> *mut T {
        *ptr = ptr.offset(-1);
        *ptr
    }

    // When dropped, copies the range `start..end` into `dest..`.
    struct MergeHole<T> {
        start: *mut T,
        end: *mut T,
        dest: *mut T,
    }

    impl<T> Drop for MergeHole<T> {
        fn drop(&mut self) {
            // `T` is not a zero-sized type, so it's okay to divide by its size.
            let len = (self.end as usize - self.start as usize) / mem::size_of::<T>();
            unsafe { ptr::copy_nonoverlapping(self.start, self.dest, len); }
        }
    }
}

#[cfg(tests)]
mod test {
    use super::*;

    #[test]
    fn mergeShouldSortCorrectly() {
        let mut target_vec = vec![1,3,5,7,2,4];
        let mid = 4;
        let mut buf = Vec::with_capacity(2);

        unsafe {
            merge(&mut target_vec, mid, buf.as_mut_ptr(), &mut |&a: i32, &b: i32| a < b);
        }

        assert_eq!(1, target_vec[0]);
        assert_eq!(2, target_vec[1]);
        assert_eq!(3, target_vec[2]);
        assert_eq!(4, target_vec[3]);
        assert_eq!(5, target_vec[4]);
        assert_eq!(7, target_vec[5]);
    }

}
