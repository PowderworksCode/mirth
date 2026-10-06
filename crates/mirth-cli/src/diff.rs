//! A unified diff of two texts, by lines.

/// The lines of `old` and `new`, aligned on their longest common
/// subsequence, with three lines of context around each change.
pub fn unified(old: &str, new: &str, name: &str) -> String {
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();
    let (n, m) = (old.len(), new.len());
    let mut longest = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            longest[i][j] = if old[i] == new[j] {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }

    let mut lines: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            lines.push((' ', old[i]));
            i += 1;
            j += 1;
        } else if i < n && (j == m || longest[i + 1][j] >= longest[i][j + 1]) {
            lines.push(('-', old[i]));
            i += 1;
        } else {
            lines.push(('+', new[j]));
            j += 1;
        }
    }

    let near_a_change = |at: usize| {
        let from = at.saturating_sub(3);
        let to = (at + 4).min(lines.len());
        lines[from..to].iter().any(|(mark, _)| *mark != ' ')
    };
    let mut out = format!("--- {name}\n+++ this build\n");
    let mut skipping = false;
    for (at, (mark, line)) in lines.iter().enumerate() {
        if near_a_change(at) {
            if skipping {
                out.push_str("@@\n");
                skipping = false;
            }
            out.push(*mark);
            out.push_str(line);
            out.push('\n');
        } else {
            skipping = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::unified;

    #[test]
    fn marks_what_changed() {
        let diff = unified("a\nb\nc\n", "a\nx\nc\n", "f");
        assert!(diff.contains("-b\n+x\n"), "{diff}");
    }
}
