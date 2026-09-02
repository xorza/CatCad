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

**A corner whose blends do not share a `d` is not this build.** Where the three
dihedrals differ the six ends stand at three distances, no one sphere holds
them, and the springs want a rule that interpolates instead. Every corner whose
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

**The split is three quads and not six.** Join every other corner of the hexagon
to a middle — the three where a spring ends and a cross arc begins — and what
falls out is three four-sided pieces, each carrying one whole spring, one whole
cross arc, and two curves in to the middle. Each piece then takes the tangency
of one blend and one face and nothing else, and the three meet across the
internal curves. That is the setback split the literature builds a vertex blend
over, at `n = 3`.

**What it owes the tier**: `at` and `normal` in closed form off the pieces;
tangency along the boundary exactly, which the construction gives rather than
fits; `uv` by the Newton solve §4.7 already pays for; `met_by` and `straying`
measured over its own extent, each carrying its bound.

**And it is the whole of the work left.** `Gusset` is eleven hundred lines of
surface and a thousand of tests for a patch with *two* sides of tangency; this
one has six and a split besides. Nothing before it is worth writing until it
stands, because a setback nothing reads and an opening with no face to raise are
both dead.

### 4 — what reads it

`Checking` holds every loop as a boundary of its own face in that face's
parameters, and the smooth flag at every edge the rounding mints — so the patch
is held to its own tangency by the checker that already exists. `Stepping`
writes a `Gusset` as a chorded net, and an n-sided patch goes out the same way.
The mesher reads `strides` and `straying`, which the patch supplies.

### 5 — what it retires

A setback vertex blend answers more than the corner it was built for: unequal
reaches meeting at a vertex, a corner of four edges, and the pinch a large reach
makes of a small face. Each of those is a refusal in §7.5's list today.

## 6. What not to do

**Do not add a fourth bespoke exact surface.** Three corner topologies have each
been answered with one, and the fourth has no such answer — that is what §2
proves four ways over. A fifth topology would ask the same question again.

**Do not fill the hole with a plane.** It is exact, cheap and wrong: a flat
facet in a rounded corner reads wrongly, and §7.7 already declines it for the
disagreeing pair by building a tangent patch instead.

**Do not run two ruled halves into each other.** Their shared ruling exists and
lands, but the sweep past it does not: §2's fourth proof is what that costs.

## 7. Read alongside

- ACIS Blending component documentation, on vertex blends of mixed convexity
  and the n-sided vertex blend surface:
  <http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF>
- *Geometric construction for setback vertex blending*, Computer-Aided Design:
  <https://www.sciencedirect.com/science/article/pii/S001044859600070X>
- *Joining smooth patches around a vertex to form a Ck surface*, CAGD:
  <https://www.sciencedirect.com/science/article/abs/pii/016783969290032K>
- *Overlap patches: a new scheme for interpolating curve networks with n-sided
  regions*, CAGD:
  <https://www.sciencedirect.com/science/article/abs/pii/016783969190046E>
