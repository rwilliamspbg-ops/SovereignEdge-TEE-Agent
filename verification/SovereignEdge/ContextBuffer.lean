/-!
# Context buffer eviction — `crates/common/src/lib.rs`

Models `ContextBuffer::push`: a FIFO of frames with a byte cap and a
frame-count cap, evicting oldest-first. Frames are abstracted to their
payload sizes (`List Nat`, oldest at the head), which is the only data
the eviction logic inspects.

The Rust loop structure is mirrored exactly:
1. evict from the front while `total_bytes + frame_size > max_bytes`,
2. evict from the front while `len ≥ max_frames`,
3. append the new frame.

Theorems:
* `push_bytes_bound` — `total_bytes ≤ max_bytes` after `push`, **provided
  the incoming frame itself fits** (`newSize ≤ maxBytes`).
* `push_frames_bound` — `len ≤ max_frames` after `push`, **provided
  `max_frames ≥ 1`**.
* `oversized_frame_breaks_bytes_bound` / `zero_capacity_breaks_frames_bound`
  — machine-checked counterexamples showing both preconditions are
  necessary: the Rust implementation admits these violations.
-/

namespace SovereignEdge.ContextBuffer

/-- Total payload bytes (`ContextBuffer::total_bytes`). -/
def total : List Nat → Nat
  | [] => 0
  | x :: xs => x + total xs

/-- First Rust loop: `while total_bytes + frame_size > max_bytes && !empty { remove(0) }` -/
def evictBytes (maxBytes newSize : Nat) : List Nat → List Nat
  | [] => []
  | x :: xs =>
    if total (x :: xs) + newSize > maxBytes then evictBytes maxBytes newSize xs
    else x :: xs

/-- Second Rust loop: `while len >= max_frames && !empty { remove(0) }` -/
def evictFrames (maxFrames : Nat) : List Nat → List Nat
  | [] => []
  | x :: xs =>
    if xs.length + 1 ≥ maxFrames then evictFrames maxFrames xs
    else x :: xs

/-- `ContextBuffer::push` (frame abstracted to its size, appended at the back). -/
def push (maxFrames maxBytes newSize : Nat) (frames : List Nat) : List Nat :=
  evictFrames maxFrames (evictBytes maxBytes newSize frames) ++ [newSize]

theorem total_append (l : List Nat) (x : Nat) : total (l ++ [x]) = total l + x := by
  induction l with
  | nil => simp [total]
  | cons y ys ih => simp [total, ih]; omega

/-- After the byte-eviction loop, either the new frame fits or the buffer
was drained empty. -/
theorem evictBytes_spec (maxBytes newSize : Nat) (l : List Nat) :
    total (evictBytes maxBytes newSize l) + newSize ≤ maxBytes ∨
    evictBytes maxBytes newSize l = [] := by
  induction l with
  | nil => right; rfl
  | cons x xs ih =>
    unfold evictBytes
    split
    · exact ih
    · left; omega

/-- Frame-count eviction only removes elements, so it never increases the
byte total. -/
theorem evictFrames_total_le (maxFrames : Nat) (l : List Nat) :
    total (evictFrames maxFrames l) ≤ total l := by
  induction l with
  | nil => simp [evictFrames]
  | cons x xs ih =>
    unfold evictFrames
    split
    · calc total (evictFrames maxFrames xs) ≤ total xs := ih
        _ ≤ x + total xs := by omega
    · simp [total]

/-- After the count-eviction loop, either there is room for one more frame
or the buffer was drained empty. -/
theorem evictFrames_spec (maxFrames : Nat) (l : List Nat) :
    (evictFrames maxFrames l).length + 1 ≤ maxFrames ∨
    evictFrames maxFrames l = [] := by
  induction l with
  | nil => right; rfl
  | cons x xs ih =>
    unfold evictFrames
    split
    · exact ih
    · left
      simp only [List.length_cons]
      omega

/-- **Byte-cap invariant.** If a frame fits within `maxBytes` on its own,
pushing it leaves the buffer within the byte cap — from any prior state. -/
theorem push_bytes_bound (maxFrames maxBytes newSize : Nat) (frames : List Nat)
    (hfit : newSize ≤ maxBytes) :
    total (push maxFrames maxBytes newSize frames) ≤ maxBytes := by
  unfold push
  rw [total_append]
  rcases evictBytes_spec maxBytes newSize frames with h | h
  · have := evictFrames_total_le maxFrames (evictBytes maxBytes newSize frames)
    omega
  · rw [h]
    simp [evictFrames, total]
    omega

/-- **Frame-cap invariant.** With a capacity of at least one frame,
pushing leaves the buffer within the frame cap — from any prior state. -/
theorem push_frames_bound (maxFrames maxBytes newSize : Nat) (frames : List Nat)
    (hcap : 1 ≤ maxFrames) :
    (push maxFrames maxBytes newSize frames).length ≤ maxFrames := by
  unfold push
  rw [List.length_append]
  rcases evictFrames_spec maxFrames (evictBytes maxBytes newSize frames) with h | h
  · simpa using h
  · rw [h]
    simpa using hcap

/-- **Counterexample (necessity of `hfit`).** A single frame larger than
`max_bytes` is admitted after draining the buffer, violating the byte cap:
this is real behavior of `ContextBuffer::push`. -/
theorem oversized_frame_breaks_bytes_bound :
    total (push 10 10 11 [5]) > 10 := by decide

/-- **Counterexample (necessity of `hcap`).** With `max_frames = 0` the
count loop drains the buffer and then appends anyway, so `len = 1 > 0`. -/
theorem zero_capacity_breaks_frames_bound :
    (push 0 100 1 []).length > 0 := by decide

end SovereignEdge.ContextBuffer
