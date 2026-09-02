# Vertex blends, and why one corner is refused

Read alongside [`KERNEL.md`](KERNEL.md) §7.5, which states the refusal, and
§7.7, which argues the corner two picks do not agree about.

## 1. What is refused

A rounding refuses a corner where three picked edges meet, the three do not
share a convexity, and the bevel is round. The flat one is answered — three
chamfer planes cross at a point whatever each was cut from — so what is missing
is one surface and not one routine.

## 2. What the corner leaves

Three blends of the one reach, and each of the three faces meeting there keeps
one corner of its own: the place its two rails cross. Every one of those three
places stands on exactly two of the three cylinders. So the hole is three-sided,
one curve across each cylinder between the two places that lie on it, and the
patch has to be tangent to all three along those three sides.

**Nothing already written reaches it**, and each of the four is a proof rather
than a search:

- **No sphere.** The patch of the agreeing corner stands on all three cylinder
  axes at once. Each axis lies a reach off the two faces its own blend divides,
  on the side its pick says — so a pair that disagrees puts its two axes on
  opposite sides of the face they share, and no point is on both.
- **No quadric.** §7.7 proves it of a patch tangent to two quadrics along
  curves, and both of the pairs that disagree here are the pair it argues about.
- **Nothing a rolling ball of the one reach sweeps**, the sphere's whole family
  and the torus with it. A ball of reach `r` touches a cylinder of reach `r`
  along a whole circle only where its centre sits on that cylinder's axis, and
  the circle is a cross section — so a side a ball could sweep runs between two
  corners standing at one place along the axis. The side on the fill does; the
  side on each cut stands its two corners a reach either way.
- **No ruled patch.** §7.7's rulings are tangent lines to the far cylinder, so
  each wants a place *outside* that cylinder to leave from. A ruled patch is
  tangent to two where the hole wants three, so however the rulings are
  organized one half falls between the two blends that agree — three pairs, two
  that disagree. And two that agree stand both axes a reach off the face they
  share on the same side: the axes are coplanar and cross, and the cylinders run
  through each other. On the notch's step corner the two cut blends' axes read
  nought apart, and over the middle of the side the hole wants, one blend's own
  surface stands between `0.897` and `0.982` off the other's axis where the
  reach is `1`. A place inside a cylinder has no tangent line to it.

## 3. The root cause

Two decisions, both deliberate, and neither wrong on its own. What is wrong is
that together they leave this corner with nothing to be filled by.

### 3.1 The corner's boundary is a consequence rather than a choice

A blend runs to the vertex. Its face is bounded by its two rails, and the rails
end where they cross another blend's — `Met::of`, one place per face. So the
patch's three sides are *whatever those crossings leave*, and the four proofs
above are all statements about that particular boundary.

Where the three picks agree it happens to be a boundary a sphere spans: the
tangency circles are cross sections of the three cylinders and they close on
each other. Where they do not, one blend's face has to reach past the end of the
face it was blended against, into the region where the next blend's cylinder
already is — and that is the interpenetration the ruled family dies on.

Nothing in `Round` can move that boundary. It carries a reach and a bevel and no
third thing.

### 3.2 The tier answered each topology with a surface of its own

§4.1 puts a surface in the exact tier or the fitted one, and §7.7 went further
for `Gusset` than either tier asked: a **closed-form inversion** and a **ray
answered to a bounded degree**, both of which a ruled surface buys by being
ruled. So the kernel's answer to each new blend topology has been a bespoke
exact surface — a sphere for the agreeing triple, a ruled patch for the
disagreeing pair, a triangle for the flat one — and that pattern has now met a
topology with no bespoke exact answer.

**The contract was never quite the obstacle it looked like.** §4.7 already said
a free-form surface costs a Newton solve and that the kernel pays it, so the
inversion was settled before this file was written. The ray was not, and stage 0
settles it. What is left is work rather than permission.

**The tier already measures.** `Gusset` answers `straying`, `chorded`, `fills`,
`nearest` and `wavering` by reading rather than by formula, and its second edge
is walked. What it does not measure is `met_by`. So the step is to widen *which*
questions an arm may answer by measuring — not to admit a new kind of answer the
tier has never given.

## 4. What the field does

The two are the same two the kernel already has, plus the one it does not.

- **A rolling-ball vertex blend.** Where the edges agree and share a radius, the
  ball pivots at the vertex and the patch is a sphere. ACIS states it plainly:
  in simple blending the geometry of a vertex blend is usually a sphere.
- **A setback vertex blend**, for everything else — mixed convexity among them,
  which ACIS blends with the three edges at one radius rather than refusing.
  The edge blends are **stopped short of the vertex**: their boundaries are
  terminated before they reach the crossings, the termination points are joined
  by *spring curves*, and a larger surface piece is inserted in the opening.
  The setback is about the distance of the cross curve from the vertex.
- **An n-sided patch** fills that opening. ACIS describes the general case as an
  n-sided vertex blend surface; the literature builds it as a `2n`-sided patch
  assembled from standard polynomial patches split at the setbacks, or as a
  Gregory patch, joined to each edge blend with tangent-plane continuity.

The reason given for setbacks is the reason this corner needs one: edges are
locally convex or concave at arbitrary angles and their blends vary widely in
cross-sectional curvature, and setbacks are what keep the corner clear of the
difficult shape configurations. Running the blends to the vertex is the choice
the field does not make.

## 5. The plan

Nothing here changes a case that works today: the sphere, the star, the junction
and the ruled patch stay exactly as they are, and the setback route is what a
corner falls to when they do not answer.

**Stage 0 stands on its own and is done.** What follows it does not — see below,
where the rest comes back together as one build.

### Stage 0 — the contract, decided

**Half of it was settled already.** §4.7 asks every surface for one
representation and no pcurves, and says outright that a Newton solve is what a
free-form surface will cost and that the kernel pays it. So `uv` was never the
question, and this file was wrong to say the contract excluded it.

**The ray was the question, and §4.7 now carries the answer.** A free-form patch
answers a ray in no closed form, and a Newton solve is no answer at all to a
question whose whole use is a *count* — the sounding reads a parity, so a
crossing missed or counted twice is a body solid to one question and hollow to
the next. What the tier admits instead is a **bounded** surface cut into pieces
until a piece's normal turns too little for any ray to meet it twice, handing
back a count; and a piece the cutting cannot separate hands back nothing, which
sends the sounder to its next cast. `Surface::met_by` has one caller in the
kernel and is asked only of a face whose own box the ray pierces.

### Stage 1 — the setback, and what derives it

**The rule this file first gave is wrong.** It said each blend's face must stop
before it reaches any other blend's cylinder. The corner three picks *agree*
about fails that rule and wants no setback at all: the sphere touches each blend
along a circle, and the arc of it the patch uses runs between the two places the
sphere touches the other two faces — which stand *on* those two blends' own
axes. Measured on a cube corner, the patch's side on one blend comes `0` from
each of the other two axes where the reach is `1`. Blends at a corner run
through each other as a matter of course, and that is not what a setback is for.

**What a setback is for is the opening.** Stopped short, each blend ends on a
cross section of itself, whose two ends stand on the two faces it divides. Two
blends reaching one face stop short of the place their rails cross, so that face
carries two distinct ends and a *spring curve* between them. Three blends leave
six sides — three cross sections and three spring curves — which is the `2n` the
literature builds a vertex blend over.

**So the constraint is one-sided, and slack.** Any setback above nought leaves
the opening well formed; a setback of nought puts the two ends of every spring
curve at one place, which is the corner as it stands today. Over it stands the
blend's own run: a blend must keep a face, so the setback and whatever the far
end takes have to fit between them — the reach that runs off the end of an edge
is a refusal §7.5 already makes.

**Which leaves the size a shape choice rather than a derivation**, and the only
length the corner is made of is the reach. A blend's rails already stand a reach
off each face it divides, so a setback of one reach along the edge stops the
cross section as far from the vertex as the rails already stand from their
faces. That is the first size to try, and stage 3 is what can measure whether
the patch wants more.

**Its code lands with stage 2.** A setback nothing acts on is a number carried
and never read, and where a blend stops is stage 2's own work.

### Stages 2 to 5 are one change

**None of them lands on its own.** A setback nothing reads is a number carried
and never asked; a surface nothing constructs is a variant that warns; an
opening with no face to raise is a body the checking refuses. So what is left is
one build, and its pieces run in dependency order rather than the order they
were first written down: the opening, then the patch that spans it, then the
topology that mints it, then what reads it. The opening is worked out and held
below; the patch is what is left.

### 2 — the opening

Each blend stops on a cross section of itself a setback `t` from the vertex,
square to its own spine, and the two ends of that section stand on the two rails
the blend has on the two faces it divides. A rail stands `d` off the edge along
its face — one reach where the two faces meet square — so a stopped end stands
`√(t² + d²)` from the vertex.

**Where the three blends share a `d`, all six ends stand at one distance**, and
that is what makes the springs write themselves: each is the arc of the sphere
of radius `ρ = √(t² + d²)` about the vertex, cut by the face it lies on, taken
the way round that stays on that face. So the opening is three plane arcs on the
blends and three sphere arcs on the faces, six sides and six corners.

**The way round is where the reflex face is answered.** A face the concave edge
does not touch turns past a half at the vertex, so a straight spring across it
leaves the face — measured at a reach of a half and a setback of one, such a
spring stands in the notch's own void over the middle three eighths of its run.
The sphere arc has two ways round and one of them stays on the face: on the
notch's step corner the two square faces take `22.6°` where the reflex one takes
`157.4°`, the long way round the corner the face keeps. Held over four reaches
and setbacks: the six ends read one distance to the last bit, every spring lies
on its own face exactly, and each face has a way round that stays on it.

**Written, and held by its own rows.** `solid::geometry::vertexed` carries the
opening: the six places off the rails, the one distance they stand at, and the
two refusals — a corner whose blends do not agree about that distance, and a
setback of the rail's own offset, which puts the two places on a face together
and leaves no spring. It is gated until the patch reads it, and says so.

**A corner whose blends do not share a `d` is not this build.** Where the three
dihedrals differ the six ends stand at three distances, no one sphere holds
them, and the springs want a rule that interpolates instead — which is the
refusal `Vertexed::opened` makes. Every corner whose
faces meet square shares a `d`, which is the corner in the issue log and every
corner an extrusion raises.

### 3 — the patch

A bounded six-sided surface in `Fitted`, spanning the opening and tangent along
every one of its sides: to a blend's cylinder along each cross arc, and to a
face's plane along each spring.

**Its boundary data is complete and it agrees at every corner.** Along a cross
arc the patch's normal is the blend cylinder's own; along a spring it is the
face's, which is one direction for the whole side. Where the two meet, they are
the *same* direction — and by a proof rather than a tolerance: a corner of the
opening is a place on the blend's own rail, and a blend is tangent to the face
along its rail. Read over the six on the notch's step corner, the blend's normal
and the face's disagree by nought.

So the opening is a G1 boundary the field's own constructions take: six curves,
six normal fields, and no corner to reconcile.

**And it crosses each face rather than touching it.** A face shared by the fill
and a cut is reached by the fill's tube from one side and the cut's from the
other: measured on the notch's step corner, the fill's cross section runs `w`
from `−0.5` to `0` and the cut's from `0` to `+0.5` of the very same plane. So
the patch joins a curve under that face to one over it and is tangent to the
face between — an inflection along the spring, not a touch. Every one-sided
family is out with it: a torus meets a plane along a circle from one side only,
and so does anything else that merely rests on one.

**And it is a graph over a sphere, which is what removes every difficulty
above.** Write the patch `c + ρ(ω)·ω` for a unit direction `ω` about a centre
`c`: its domain is that sphere's own two angles, so it needs no hexagonal domain
and no split into quads; its face is the hexagonal *region* of that domain,
trimmed as every face here is trimmed; and there are no seams, so no curve with
a tangent prescribed at both ends and no vertex enclosure to satisfy. `uv` is
then the direction of `at − c`, inverted the way a sphere is — closed form, and
§4.7's Newton solve is not even spent.

**And there is no such `c`.** A graph about a place reads its normal as
`ρω − ∇ρ`, so along the radial it reads `ρ` and never nought — which means the
patch's own normal has to keep one sign against the way the centre looks at it,
all the way round the boundary. The corner fails outright: the radial at a
spring lies *in* the face that spring is on, so both ends of the floor's spring
read `0.000` against its normal. And nowhere else answers either. Searched over
41³ places out to three reaches of the notch's step corner, the best margin any
centre reaches is `−0.203`, and the best of them is the corner itself.

**So the patch is no graph over any sphere.** Its normals swing too far between
the fill's side and the cuts', and no one place sees them all on one side. `uv`
does not come out in closed form that way, and §4.7's Newton solve is what the
surface costs after all.

**Held on the notch's step corner**: the boundary stands between `1.0212` and
`1.1180` from the corner where the six places stand `1.1180`, so every ray from
the corner meets it once; the nearest two of the six directions are `36.9°`
apart; and no two places of the boundary share a direction.

**But it is a graph over a *plane*, and that is what the surface is.** The
condition is weaker than the sphere's and it holds: the patch takes a blend's
normal along each cross section and a face's along each spring, so what its
normal does over the whole boundary is swing between the three faces — and their
sum is the direction that swing leans about. Read on the notch's step corner,
the whole boundary stands `1/√3` against it at worst, and a search over four
hundred thousand directions finds none better than the three faces' own sum.

**Which buys the contract back.** A place inverts by flattening on to that
plane, so `uv` is closed form; the domain is the plane's own two, so no hexagon
is wanted and no split into quads; and there are no seams, so no curve with a
tangent at both ends and no vertex enclosure. The face is the hexagonal *region*
of the domain, trimmed as every face here is, and its six sides are circles the
kernel already writes.

**So what was left to write was a scalar, and it is written.** The height
carries a value and its *whole* gradient along each of the six sides — a height
field faces `(−h_x, −h_y, 1)` in its own plane's frame, so a normal prescribed
along a side fixes both readings of the gradient there and not just the one
across. `Vertexed::heighted` writes that down and `Patched` blends it: each side
carries its own reading to a place inside, weighed by one over the square of how
far that side stands from it in the plane. A place *on* a side reads that side's
own numbers back, the other five weighing nothing against them.

**Held on the notch's step corner.** The patch meets every one of its six sides
in place and in facing to `1e-12`, which is the tangency the whole corner exists
for; it reads its own parameters back off any place; and walked from the middle
out to each of the six corners it keeps facing the way its own plane does, so
the domain names one place of it and not two.

**A footing is walked and not solved.** A circle flattens to an ellipse and the
nearest place on one is a quartic, so the blend finds each side's own footing by
a fan of twenty-four angles and forty halvings — `Gusset::nearest`'s own shape,
and the tier's own habit.

**And it owes the tier nothing more.** `solid::geometry::vertexed` answers every
question `Fitted` asks: `at` lifts a domain place by the blended height and `uv`
flattens on to the plane, both closed form; `normal` reads the height's own
gradient by a central difference at a millionth of the reach, so it is always
the gradient of the height the patch actually has; `met_by` walks the stretch of
a ray that runs within two reaches of the corner and closes on each sign change
by halving, which is the counted answer stage 0 admits; and `off`, `nearest`,
`fills`, `spans`, `wavering`, `straying`, `strides` and the STEP net are read
over the patch's own extent, as this tier's other arm already reads its own.

**It is a `Fitted` arm and it is small enough to be one.** A `Surface` is copied
by value on every path a frame walks, so the patch holds three *lines* and three
normals rather than three cylinders and three planes — the three blends sharing
one reach and none of them wanting a frame, and all three faces running through
the corner. That keeps `Surface` inside what the largest arm may stand over the
rest.

### 4 — the rounding that raises one

**What is left, and it is topology rather than geometry.** The planning has to
carry a corner the picks disagree about instead of refusing it, and the minting
has to put six corners, six edges and one face where the refusal is now.

**The planning.** `Trihedral::of` already answers for the corner; what follows
it is `Trihedral::outward` refusing. Where it does, build a `Vertexed` instead:
the three axes are the blends' own, pointed away from the corner off each edge's
far end; the three normals are the shared faces' own, out of the material;
`shared[i]` is the face that `ends[i]` and `ends[i + 1]` both divide, the one
their two `Spine::between` pairs have in common. The reach is the blends'; the
setback is *twice* it, one reach being where the springs vanish.
`Vertexed::opened` is then the refusal for a corner this does not span, and the
answer is a `Filled::Vertexed` beside the star and the sphere.

**The minting**, on `Rounding::ring`'s own shape. Six corners from
`Opened::made`; three cross sections, each between one blend's face and the
patch's; three springs, each between one of the three faces and the patch's. A
blend closes on its own cross section where it closes on an arc today —
`Ending::Vertexed`, read by `Rounding::closes` — and `Rounding::ended` gives its
rail's corner as `made[i][0]` where the rail's face is the one before it and
`made[i][1]` where it is the one after. `Rounding::line` puts each spring into
its own face's loop at the corner, which is where the ruled patch's straight
side already goes in.

**The face faces out.** The patch's own plane normal is the three faces' normals
added, each out of the material — so the patch's normal is too, and `outward` is
true.

**The winding is what to hold to `Checking`.** Every loop of every face is
re-derived there against the face's own parameters, so a spring or a cross
section walked the wrong way is caught rather than shipped.

**Written, and one thing short of standing.** `Planning::vertexing`,
`Rounding::span` and the loop that walks the six are in, and the body they build
passes `Checking` outright — every coedge paired, every loop bounding its own
face, the shells and the genus all as they should be. The patch's own loop runs
the six *backwards*, which is the one bit the winding turned out to want.

**What it fails is the mesh.** A face is cut into cells to a stride and every
triangle is then held within a sagitta of the surface, and the patch's own
straying does not fall under one however finely the cells are cut — not at a
stride halved ten times, and not with the triangle probed at a hundred places.
Two things are likely in it, and neither is measured yet: the blend is a
weighted sum of six footings and a footing *jumps* where the nearest place on a
side changes branch, which is a kink no cell size mends; and the domain runs
over the whole plane, so a cell the trimming leaves along the boundary reads the
blend where it is extrapolating rather than interpolating.

**The footing is closed form now, and one of the two is mended.** A circle
flattens to `A + B·cos θ + C·sin θ`, so a place of that ellipse reads
`(cos θ, sin θ)` in the basis `B` and `C` — and taking the bearing of `uv` in
that basis hands back the side's own parameter where `uv` stands on the side. No
walk, no jump, and a mesh of one patch that ran to seventy seconds runs in five.

**And each side is held to its own stretch.** A circle runs on past its side and
its flattened image runs on through the middle of the opening, so a place inside
could stand *on* the continuation of a side it is nowhere near and the blend
would snap to that side's reading. Clamped, the reading comes to nought on the
side and nowhere else.

**The patch is smooth now, and the ridge that stopped it is named.** A place
whose bearing leaves a side's stretch is held to whichever of the two ends is
nearer, and half a turn from the arc's middle the nearer end changes over — so
the held place stands as far either way and leans the other. That is a ridge,
and the springs run far enough round their own circle that the ridge crossed
the opening. Measured on it, the second difference of the height doubled at
every halving of the step, which is a gradient that jumps.

**Two changes close it.** A side's height and slope are read where the bearing
puts it on that side's *circle*, which moves smoothly and runs on past the
side's own ends, while how far the side stands is read from the place held to
its stretch — so the run past the ends weighs less without kinking the reading.
And a side's weight now tapers to nothing over a margin beyond either end, that
margin held short of the turnover, so the ridge stands where nothing weighs.

**The readings say it worked.** The height's second difference is bounded and
settled at every place probed — about `0.8` and `4.2` in the two parameters, the
same from four scattered places. The worst triangle now strays `3.61e-2`,
`1.28e-2`, `3.91e-3`, `1.07e-3` and `2.83e-4` as the cell halves: ratios `2.81`,
`3.28`, `3.64`, `3.80`, converging on `4`. That is `h²`, which is what a smooth
surface owes a mesh.

**Two more things stood between the patch and a body, and both are closed.** A
mesh lays its grid over the box the opening bounds, and that box came off the
six corners — but every side is an arc, and an arc stands off its own chord, so
the box left out a rim of the opening. The rim is where the patch bends
hardest. The box is now solved off the six arcs rather than walked: a flattened
circle is `m + a·cos θ + u·sin θ`, so it reaches furthest where `tan θ = u/a` in
each coordinate, and the two ends stand in for whichever of those four turns the
stretch does not reach.

**And the three springs share a centre, which is the corner itself.** That
centre falls on the opening's own rim, and a bearing taken about it means
nothing there — every place of the circle is as near as every other, so the
reading spins and the patch bent by thousands beside it. A side is now hushed as
a place approaches its own flattened middle, by a ratio of polynomials whose
zero is of the fourth order — which is what leaves the product of the hush and
the spinning reading flat. That one change took the corner from unmeshable to
meshed, and the test from `224 s` to `5 s`.

**The stride is worked out and no longer searched for.** A quadratic stands off
the plane through a triangle's three corners by at most an eighth of its
curvature times the longest side squared, and a cell's longest side is its
diagonal — so the straying is `κ·stride²/4` and the stride that holds it to the
sagitta is `√(4·sagitta/κ)`. The curvature is read off differences over a walk
of the six sides and in toward the middle by squares, the walk clustered where
the patch bends most. A search over cells reads what the grid does at one stride
and says nothing about the next, and it cost a walk of the whole opening at
every halving.

**What the answer is.** Three rounds that do not agree about a corner now leave
twelve faces, thirty edges and twenty corners, which Euler holds to a ball: the
notch's eight faces and a blend apiece is eleven, the same count the chamfered
answer has, and the patch is the twelfth — where three chamfers leave a point on
three legs.

**What it costs, and where that went.** The curvature walk is thousands of
readings of the height, and it used to run once for every stride a mesher asked
for — so it is now worked out where the surface is made and carried on it, which
is what makes [`Vertexed::new`] the only way one exists. A reading of the height
does no flattening and no normalising: a side's bearing hands back one turn, and
one sine and cosine of it give the place, its image in the plane and the way the
patch faces there. A triangle is probed at fifteen places rather than sixty-six,
the patch being smooth enough that a bowl has one low place. Together those took
the test from fourteen seconds to under four.

**What is left costs, and both are measured.** The six sides are worked out
again at every surface query, about once for every ten readings of the height.
And the patch bends against its own rim some twenty to forty times harder than
its reach accounts for — which is what sizes the grid, the cells going as the
curvature. Neither is the corner being wrong; both are in `.notes/ISSUES.md`.
