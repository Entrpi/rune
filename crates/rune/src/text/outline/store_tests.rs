use super::*;

/// Hint records until the store has no room, each three texels.
fn fill(store: &mut OutlineStore) -> u32 {
    (1..).find(|&n| store.hint([f64::from(n); 11]) == 0).unwrap() - 1
}

/// A GPU whose textures are shorter than the store's budget holds it to them: records past
/// its rows don't fit, the rows it uploads stay within them, and one that grew taller before
/// it knew starts again within them at the next frame.
#[test]
fn a_store_holds_to_the_rows_its_gpu_takes() {
    let mut store = OutlineStore::default();
    store.set_max_rows(2);
    let fitted = fill(&mut store);
    assert_eq!(fitted, (2 * WIDTH - 1) / 3, "records fill two rows and no more");
    assert!(store.rows() <= 2);
    let (y, bytes) = store.take_dirty().unwrap();
    assert!(y as usize * WIDTH as usize * 16 + bytes.len() <= 2 * WIDTH as usize * 16);

    let mut store = OutlineStore::default();
    for n in 0..WIDTH - 1 { store.hint([f64::from(n); 11]); }
    assert_eq!(store.rows(), 3);
    store.set_max_rows(2);
    let (y, bytes) = store.take_dirty().unwrap();
    assert_eq!((y, bytes.len()), (0, 2 * WIDTH as usize * 16), "rows past the GPU's are never uploaded");
    assert!(store.begin_frame(false), "it starts again");
    assert!(store.rows() <= 2);
}
