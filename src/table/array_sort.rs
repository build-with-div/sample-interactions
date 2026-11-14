//! `Array.prototype.sort` as V8 implements it (`third_party/v8/builtins/
//! array-sort.tq`): TimSort, after CPython's `listsort`.
//!
//! ECMAScript leaves the order implementation-defined when the comparator
//! is inconsistent, and table-core's comparators are inconsistent whenever a
//! column holds `NaN` or values of mixed types (`compareBasic(NaN, x)` and
//! `compareBasic(x, NaN)` are both -1). Chromium's order is then V8's, so
//! the port runs V8's algorithm step for step: the same comparisons, in the
//! same order, with the same galloping thresholds. Rust's `sort_by` would
//! give another order there, and may panic on a comparator that isn't a
//! total order.

/// V8's `kMinGallopWins`, the initial `minGallop`.
const MIN_GALLOP: usize = 7;

/// Sorts `items` in place as `items.sort(compare)` does in V8. `compare`
/// returns the comparator's number; `NaN` counts as 0, as `ToNumber` then
/// V8's `if (NumberIsNaN(v)) return 0` treat it.
pub fn sort<T: Clone>(items: &mut [T], mut compare: impl FnMut(&T, &T) -> f64) {
    let length = items.len();
    if length < 2 {
        return;
    }
    let mut state = State {
        work: items.to_vec(),
        runs: Vec::new(),
        min_gallop: MIN_GALLOP,
        compare: &mut compare,
    };
    let mut low = 0;
    let mut remaining = length;
    let min_run = min_run_length(length);
    while remaining != 0 {
        let mut run = state.count_and_make_run(low, low + remaining);
        if run < min_run {
            let forced = min_run.min(remaining);
            state.binary_insertion_sort(low, low + run, low + forced);
            run = forced;
        }
        state.runs.push((low, run));
        state.merge_collapse();
        low += run;
        remaining -= run;
    }
    state.merge_force_collapse();
    items.clone_from_slice(&state.work);
}

/// `ComputeMinRunLength`.
fn min_run_length(mut n: usize) -> usize {
    let mut r = 0;
    while n >= 64 {
        r |= n & 1;
        n >>= 1;
    }
    n + r
}

struct State<'a, T, F: FnMut(&T, &T) -> f64> {
    work: Vec<T>,
    /// Pending runs: (base, length).
    runs: Vec<(usize, usize)>,
    min_gallop: usize,
    compare: &'a mut F,
}

impl<T: Clone, F: FnMut(&T, &T) -> f64> State<'_, T, F> {
    /// `sortState.Compare(x, y) < 0` and friends read the sign of this.
    fn order(&mut self, x: &T, y: &T) -> f64 {
        let value = (self.compare)(x, y);
        if value.is_nan() {
            0.0
        } else {
            value
        }
    }

    /// `CountAndMakeRun`: the length of the run starting at `low`, made
    /// ascending (a strictly descending run is reversed).
    fn count_and_make_run(&mut self, low_arg: usize, high: usize) -> usize {
        let low = low_arg + 1;
        if low == high {
            return 1;
        }
        let mut run_length = 2;
        let element_low = self.work[low].clone();
        let element_low_pre = self.work[low - 1].clone();
        let order = self.order(&element_low, &element_low_pre);
        let descending = order < 0.0;
        let mut previous = element_low;
        for index in low + 1..high {
            let current = self.work[index].clone();
            let order = self.order(&current, &previous);
            if descending {
                if order >= 0.0 {
                    break;
                }
            } else if order < 0.0 {
                break;
            }
            previous = current;
            run_length += 1;
        }
        if descending {
            self.work[low_arg..low_arg + run_length].reverse();
        }
        run_length
    }

    /// `BinaryInsertionSort`: `[low, start)` is sorted; inserts the rest of
    /// `[low, high)`.
    fn binary_insertion_sort(&mut self, low: usize, start_arg: usize, high: usize) {
        let mut start = if low == start_arg {
            start_arg + 1
        } else {
            start_arg
        };
        while start < high {
            let mut left = low;
            let mut right = start;
            let pivot = self.work[start].clone();
            while left < right {
                let mid = left + ((right - left) >> 1);
                let mid_element = self.work[mid].clone();
                if self.order(&pivot, &mid_element) < 0.0 {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            let mut p = start;
            while p > left {
                self.work[p] = self.work[p - 1].clone();
                p -= 1;
            }
            self.work[left] = pivot;
            start += 1;
        }
    }

    fn run_length(&self, n: usize) -> usize {
        self.runs[n].1
    }

    /// `RunInvariantEstablished`.
    fn invariant(&self, n: usize) -> bool {
        if n < 2 {
            return true;
        }
        self.run_length(n - 2) > self.run_length(n - 1) + self.run_length(n)
    }

    /// `MergeCollapse`.
    fn merge_collapse(&mut self) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if !self.invariant(n + 1) || !self.invariant(n) {
                if self.run_length(n - 1) < self.run_length(n + 1) {
                    n -= 1;
                }
                self.merge_at(n);
            } else if self.run_length(n) <= self.run_length(n + 1) {
                self.merge_at(n);
            } else {
                break;
            }
        }
    }

    /// `MergeForceCollapse`.
    fn merge_force_collapse(&mut self) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if n > 0 && self.run_length(n - 1) < self.run_length(n + 1) {
                n -= 1;
            }
            self.merge_at(n);
        }
    }

    /// `MergeAt`: merges runs `i` and `i + 1`.
    fn merge_at(&mut self, i: usize) {
        let (mut base_a, mut length_a) = self.runs[i];
        let (base_b, mut length_b) = self.runs[i + 1];
        self.runs[i].1 = length_a + length_b;
        self.runs.remove(i + 1);

        let key_right = self.work[base_b].clone();
        let k = self.gallop_right(Source::Work, &key_right, base_a, length_a, 0);
        base_a += k;
        length_a -= k;
        if length_a == 0 {
            return;
        }
        let key_left = self.work[base_a + length_a - 1].clone();
        length_b = self.gallop_left(Source::Work, &key_left, base_b, length_b, length_b - 1);
        if length_b == 0 {
            return;
        }
        if length_a <= length_b {
            self.merge_low(base_a, length_a, base_b, length_b);
        } else {
            self.merge_high(base_a, length_a, base_b, length_b);
        }
    }

    fn element(&self, source: Source<'_, T>, index: usize) -> T {
        match source {
            Source::Work => self.work[index].clone(),
            Source::Temp(temp) => temp[index].clone(),
        }
    }

    /// `GallopLeft`: where `key` goes in the sorted `[base, base + length)`,
    /// left of any equal elements, searching out from `hint`.
    fn gallop_left(
        &mut self,
        source: Source<'_, T>,
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
    ) -> usize {
        let mut last_offset: isize = 0;
        let mut offset: isize = 1;
        let hint_element = self.element(source, base + hint);
        let (hint, length) = (hint as isize, length as isize);
        if self.order(&hint_element, key) < 0.0 {
            // a[hint] < key: gallop right.
            let max_offset = length - hint;
            while offset < max_offset {
                let element = self.element(source, (base as isize + hint + offset) as usize);
                if self.order(&element, key) >= 0.0 {
                    break;
                }
                last_offset = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(max_offset);
            last_offset += hint;
            offset += hint;
        } else {
            // key <= a[hint]: gallop left.
            let max_offset = hint + 1;
            while offset < max_offset {
                let element = self.element(source, (base as isize + hint - offset) as usize);
                if self.order(&element, key) < 0.0 {
                    break;
                }
                last_offset = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(max_offset);
            let previous = last_offset;
            last_offset = hint - offset;
            offset = hint - previous;
        }
        last_offset += 1;
        while last_offset < offset {
            let m = last_offset + ((offset - last_offset) >> 1);
            let element = self.element(source, (base as isize + m) as usize);
            if self.order(&element, key) < 0.0 {
                last_offset = m + 1;
            } else {
                offset = m;
            }
        }
        offset as usize
    }

    /// `GallopRight`: as `gallop_left`, right of any equal elements.
    fn gallop_right(
        &mut self,
        source: Source<'_, T>,
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
    ) -> usize {
        let mut last_offset: isize = 0;
        let mut offset: isize = 1;
        let hint_element = self.element(source, base + hint);
        let (hint, length) = (hint as isize, length as isize);
        if self.order(key, &hint_element) < 0.0 {
            // key < a[hint]: gallop left.
            let max_offset = hint + 1;
            while offset < max_offset {
                let element = self.element(source, (base as isize + hint - offset) as usize);
                if self.order(key, &element) >= 0.0 {
                    break;
                }
                last_offset = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(max_offset);
            let previous = last_offset;
            last_offset = hint - offset;
            offset = hint - previous;
        } else {
            // a[hint] <= key: gallop right.
            let max_offset = length - hint;
            while offset < max_offset {
                let element = self.element(source, (base as isize + hint + offset) as usize);
                if self.order(key, &element) < 0.0 {
                    break;
                }
                last_offset = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(max_offset);
            last_offset += hint;
            offset += hint;
        }
        last_offset += 1;
        while last_offset < offset {
            let m = last_offset + ((offset - last_offset) >> 1);
            let element = self.element(source, (base as isize + m) as usize);
            if self.order(key, &element) < 0.0 {
                offset = m;
            } else {
                last_offset = m + 1;
            }
        }
        offset as usize
    }

    /// `MergeLow`: merges with run A (the shorter) copied aside, front to
    /// back.
    fn merge_low(
        &mut self,
        base_a: usize,
        length_a_arg: usize,
        base_b: usize,
        length_b_arg: usize,
    ) {
        let mut length_a = length_a_arg;
        let mut length_b = length_b_arg;
        let temp: Vec<T> = self.work[base_a..base_a + length_a].to_vec();
        let mut dest = base_a;
        let mut cursor_temp = 0;
        let mut cursor_b = base_b;
        self.work[dest] = self.work[cursor_b].clone();
        dest += 1;
        cursor_b += 1;

        enum Exit {
            Succeed,
            CopyB,
        }
        let exit = 'merge: {
            length_b -= 1;
            if length_b == 0 {
                break 'merge Exit::Succeed;
            }
            if length_a == 1 {
                break 'merge Exit::CopyB;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let mut wins_a = 0;
                let mut wins_b = 0;
                loop {
                    let b = self.work[cursor_b].clone();
                    if self.order(&b, &temp[cursor_temp]) < 0.0 {
                        self.work[dest] = b;
                        dest += 1;
                        cursor_b += 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 0 {
                            break 'merge Exit::Succeed;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    } else {
                        self.work[dest] = temp[cursor_temp].clone();
                        dest += 1;
                        cursor_temp += 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 1 {
                            break 'merge Exit::CopyB;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP || wins_b >= MIN_GALLOP || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;

                    let key = self.work[cursor_b].clone();
                    wins_a = self.gallop_right(Source::Temp(&temp), &key, cursor_temp, length_a, 0);
                    if wins_a > 0 {
                        self.work[dest..dest + wins_a]
                            .clone_from_slice(&temp[cursor_temp..cursor_temp + wins_a]);
                        dest += wins_a;
                        cursor_temp += wins_a;
                        length_a -= wins_a;
                        if length_a == 1 {
                            break 'merge Exit::CopyB;
                        }
                        // Impossible with a consistent comparator, which
                        // table-core's aren't always.
                        if length_a == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[dest] = self.work[cursor_b].clone();
                    dest += 1;
                    cursor_b += 1;
                    length_b -= 1;
                    if length_b == 0 {
                        break 'merge Exit::Succeed;
                    }

                    let key = temp[cursor_temp].clone();
                    wins_b = self.gallop_left(Source::Work, &key, cursor_b, length_b, 0);
                    if wins_b > 0 {
                        for i in 0..wins_b {
                            self.work[dest + i] = self.work[cursor_b + i].clone();
                        }
                        dest += wins_b;
                        cursor_b += wins_b;
                        length_b -= wins_b;
                        if length_b == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[dest] = temp[cursor_temp].clone();
                    dest += 1;
                    cursor_temp += 1;
                    length_a -= 1;
                    if length_a == 1 {
                        break 'merge Exit::CopyB;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match exit {
            Exit::Succeed => {
                if length_a > 0 {
                    self.work[dest..dest + length_a]
                        .clone_from_slice(&temp[cursor_temp..cursor_temp + length_a]);
                }
            }
            Exit::CopyB => {
                // The last element of run A belongs at the end of the merge.
                for i in 0..length_b {
                    self.work[dest + i] = self.work[cursor_b + i].clone();
                }
                self.work[dest + length_b] = temp[cursor_temp].clone();
            }
        }
    }

    /// `MergeHigh`: merges with run B (the shorter) copied aside, back to
    /// front.
    fn merge_high(
        &mut self,
        base_a: usize,
        length_a_arg: usize,
        base_b: usize,
        length_b_arg: usize,
    ) {
        let mut length_a = length_a_arg as isize;
        let mut length_b = length_b_arg as isize;
        let temp: Vec<T> = self.work[base_b..base_b + length_b_arg].to_vec();
        let base_a = base_a as isize;
        let mut dest = base_b as isize + length_b - 1;
        let mut cursor_temp = length_b - 1;
        let mut cursor_a = base_a + length_a - 1;
        self.work[dest as usize] = self.work[cursor_a as usize].clone();
        dest -= 1;
        cursor_a -= 1;

        enum Exit {
            Succeed,
            CopyA,
        }
        let exit = 'merge: {
            length_a -= 1;
            if length_a == 0 {
                break 'merge Exit::Succeed;
            }
            if length_b == 1 {
                break 'merge Exit::CopyA;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let mut wins_a: isize = 0;
                let mut wins_b: isize = 0;
                loop {
                    let a = self.work[cursor_a as usize].clone();
                    if self.order(&temp[cursor_temp as usize], &a) < 0.0 {
                        self.work[dest as usize] = a;
                        dest -= 1;
                        cursor_a -= 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 0 {
                            break 'merge Exit::Succeed;
                        }
                        if wins_a as usize >= min_gallop {
                            break;
                        }
                    } else {
                        self.work[dest as usize] = temp[cursor_temp as usize].clone();
                        dest -= 1;
                        cursor_temp -= 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 1 {
                            break 'merge Exit::CopyA;
                        }
                        if wins_b as usize >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a as usize >= MIN_GALLOP || wins_b as usize >= MIN_GALLOP || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;

                    let key = temp[cursor_temp as usize].clone();
                    let k = self.gallop_right(
                        Source::Work,
                        &key,
                        base_a as usize,
                        length_a as usize,
                        length_a as usize - 1,
                    ) as isize;
                    wins_a = length_a - k;
                    if wins_a > 0 {
                        dest -= wins_a;
                        cursor_a -= wins_a;
                        for i in (0..wins_a).rev() {
                            self.work[(dest + 1 + i) as usize] =
                                self.work[(cursor_a + 1 + i) as usize].clone();
                        }
                        length_a -= wins_a;
                        if length_a == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[dest as usize] = temp[cursor_temp as usize].clone();
                    dest -= 1;
                    cursor_temp -= 1;
                    length_b -= 1;
                    if length_b == 1 {
                        break 'merge Exit::CopyA;
                    }

                    let key = self.work[cursor_a as usize].clone();
                    let k = self.gallop_left(
                        Source::Temp(&temp),
                        &key,
                        0,
                        length_b as usize,
                        length_b as usize - 1,
                    ) as isize;
                    wins_b = length_b - k;
                    if wins_b > 0 {
                        dest -= wins_b;
                        cursor_temp -= wins_b;
                        for i in 0..wins_b {
                            self.work[(dest + 1 + i) as usize] =
                                temp[(cursor_temp + 1 + i) as usize].clone();
                        }
                        length_b -= wins_b;
                        if length_b == 1 {
                            break 'merge Exit::CopyA;
                        }
                        // Impossible with a consistent comparator.
                        if length_b == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[dest as usize] = self.work[cursor_a as usize].clone();
                    dest -= 1;
                    cursor_a -= 1;
                    length_a -= 1;
                    if length_a == 0 {
                        break 'merge Exit::Succeed;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match exit {
            Exit::Succeed => {
                if length_b > 0 {
                    let start = dest - (length_b - 1);
                    for i in 0..length_b {
                        self.work[(start + i) as usize] = temp[i as usize].clone();
                    }
                }
            }
            Exit::CopyA => {
                // The first element of run B belongs at the front of the merge.
                dest -= length_a;
                cursor_a -= length_a;
                for i in (0..length_a).rev() {
                    self.work[(dest + 1 + i) as usize] =
                        self.work[(cursor_a + 1 + i) as usize].clone();
                }
                self.work[dest as usize] = temp[cursor_temp as usize].clone();
            }
        }
    }
}

enum Source<'t, T> {
    Work,
    Temp(&'t [T]),
}

// Not derived: the derive would require `T: Copy`.
impl<T> Clone for Source<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Source<'_, T> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed >> 33
    }

    #[test]
    fn sorts_like_a_stable_sort_when_the_comparator_is_consistent() {
        let mut seed = 7;
        for length in [0, 1, 2, 5, 63, 64, 65, 200, 1000, 5000] {
            let items: Vec<(u64, usize)> = (0..length).map(|i| (lcg(&mut seed) % 50, i)).collect();
            let mut ours = items.clone();
            sort(&mut ours, |a, b| a.0 as f64 - b.0 as f64);
            let mut expected = items.clone();
            expected.sort_by_key(|item| item.0);
            assert_eq!(ours, expected, "length {length}");
        }
    }

    #[test]
    fn survives_an_inconsistent_comparator() {
        let mut seed = 11;
        let mut items: Vec<u64> = (0..3000).map(|_| lcg(&mut seed)).collect();
        let mut original = items.clone();
        sort(&mut items, |_, _| {
            if lcg(&mut seed).is_multiple_of(2) {
                -1.0
            } else {
                1.0
            }
        });
        items.sort_unstable();
        original.sort_unstable();
        assert_eq!(items, original, "a permutation of the input");
    }
}
