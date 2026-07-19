; Machine-checked obligations for the algebra implemented in src/cost.rs.
; Run through proofs/check-cost-model-laws.sh. Every check must be unsat:
; an unsat result means no counterexample exists over the declared domain.

(set-logic ALL)

; The secondary component is a bounded natural with saturating addition.
(define-fun badd ((x Int) (y Int) (cap Int)) Int
  (ite (<= (+ x y) cap) (+ x y) cap))

; Costs are lexicographically ordered pairs. The first component is an
; unbounded natural; the second is a bounded natural.
(define-fun lex-le ((ao Int) (ab Int) (bo Int) (bb Int)) Bool
  (or (< ao bo) (and (= ao bo) (<= ab bb))))

(define-fun over ((column Int) (page-width Int)) Int
  (ite (> column page-width) (- column page-width) 0))

(define-fun potential ((column Int) (page-width Int)) Int
  (* (over column page-width) (over column page-width)))

(define-fun text-cost ((column Int) (run-width Int) (page-width Int)) Int
  (- (potential (+ column run-width) page-width)
     (potential column page-width)))

(define-fun consumer-penalty ((amount Int) (context Int)) Int amount)
(define-fun research-penalty ((amount Int) (context Int)) Int 0)

(echo "law 1: zero is a two-sided identity")
(push)
(declare-const io Int)
(declare-const ib Int)
(declare-const icap Int)
(assert (and (>= io 0) (>= icap 0) (>= ib 0) (<= ib icap)))
(assert
  (or (not (= (+ 0 io) io))
      (not (= (+ io 0) io))
      (not (= (badd 0 ib icap) ib))
      (not (= (badd ib 0 icap) ib))))
(check-sat)
(pop)

(echo "law 2: addition is associative")
(push)
(declare-const ao Int)
(declare-const bo Int)
(declare-const co Int)
(declare-const ab Int)
(declare-const bb Int)
(declare-const cb Int)
(declare-const acap Int)
(assert
  (and (>= ao 0) (>= bo 0) (>= co 0)
       (>= acap 0)
       (>= ab 0) (<= ab acap)
       (>= bb 0) (<= bb acap)
       (>= cb 0) (<= cb acap)))
(assert
  (or (not (= (+ (+ ao bo) co) (+ ao (+ bo co))))
      (not (= (badd (badd ab bb acap) cb acap)
              (badd ab (badd bb cb acap) acap)))))
(check-sat)
(pop)

(echo "law 3a: addition is monotone in its left argument")
(push)
(declare-const lao Int)
(declare-const lab Int)
(declare-const lbo Int)
(declare-const lbb Int)
(declare-const lco Int)
(declare-const lcb Int)
(declare-const lcap Int)
(assert
  (and (>= lao 0) (>= lbo 0) (>= lco 0)
       (>= lcap 0)
       (>= lab 0) (<= lab lcap)
       (>= lbb 0) (<= lbb lcap)
       (>= lcb 0) (<= lcb lcap)
       (lex-le lao lab lbo lbb)))
(assert
  (not (lex-le (+ lao lco) (badd lab lcb lcap)
               (+ lbo lco) (badd lbb lcb lcap))))
(check-sat)
(pop)

(echo "law 3b: addition is monotone in its right argument")
(push)
(declare-const rao Int)
(declare-const rab Int)
(declare-const rbo Int)
(declare-const rbb Int)
(declare-const rco Int)
(declare-const rcb Int)
(declare-const rcap Int)
(assert
  (and (>= rao 0) (>= rbo 0) (>= rco 0)
       (>= rcap 0)
       (>= rab 0) (<= rab rcap)
       (>= rbb 0) (<= rbb rcap)
       (>= rcb 0) (<= rcb rcap)
       (lex-le rao rab rbo rbb)))
(assert
  (not (lex-le (+ rco rao) (badd rcb rab rcap)
               (+ rco rbo) (badd rcb rbb rcap))))
(check-sat)
(pop)

(echo "law 4: text cost is incremental under splitting")
(push)
(declare-const scol Int)
(declare-const sw1 Int)
(declare-const sw2 Int)
(declare-const spage Int)
(assert (and (>= scol 0) (>= sw1 0) (>= sw2 0) (>= spage 0)))
(assert
  (not (= (text-cost scol (+ sw1 sw2) spage)
          (+ (text-cost scol sw1 spage)
             (text-cost (+ scol sw1) sw2 spage)))))
(check-sat)
(pop)

(echo "law 5: text cost is nondecreasing in starting column")
(push)
(declare-const c1 Int)
(declare-const c2 Int)
(declare-const cw Int)
(declare-const cpage Int)
(assert
  (and (>= c1 0) (>= c2 0) (<= c1 c2)
       (>= cw 0) (>= cpage 0)))
(assert (> (text-cost c1 cw cpage) (text-cost c2 cw cpage)))
(check-sat)
(pop)

(echo "law 6: primitive costs are nonnegative and penalties are context-free")
(push)
(declare-const ncol Int)
(declare-const nw Int)
(declare-const npage Int)
(declare-const namount Int)
(declare-const nnewline Int)
(declare-const ncontext1 Int)
(declare-const ncontext2 Int)
(assert
  (and (>= ncol 0) (>= nw 0) (>= npage 0)
       (>= namount 0) (>= nnewline 0)
       (>= ncontext1 0) (>= ncontext2 0)))
; Consumer penalty is (0, amount); OverflowThenHeight penalty is (0, 0).
; Neither expression contains a column or chunking context.
(assert
  (or (< (text-cost ncol nw npage) 0)
      (not (lex-le 0 0 0 nnewline))
      (not (lex-le 0 0 0 namount))
      (not (= (consumer-penalty namount ncontext1)
              (consumer-penalty namount ncontext2)))
      (not (= (research-penalty namount ncontext1)
              (research-penalty namount ncontext2)))))
(check-sat)
(pop)

(echo "implementation: u32 overflow square fits the intermediate u64")
(push)
(declare-const ustart Int)
(declare-const uend Int)
(assert
  (and (>= ustart 0) (<= ustart uend)
       (<= uend 4294967295)))
(assert
  (or (> (* uend uend) 18446744073709551615)
      (< (- (* uend uend) (* ustart ustart)) 0)
      (> (- (* uend uend) (* ustart ustart))
         18446744073709551615)))
(check-sat)
(pop)
