# Writing tour cards

How the text of a guided tour is written. It applies to every file in this folder. (Decided
2026-10-27, after a read-through found the cards hard to follow: compressed, full of coined
images and idioms, and describing things the screen did not show.)

## Who reads the cards

Someone curious, often reading English as a second language, who has never heard of the
Mandelbrot set. They read a card while looking at a busy screen: the picture, the marker, the
dots, the mini-map, maybe a panel. They cannot ask what a sentence means.

## The rules

1. **Plain English, intermediate level.** Common words, short sentences, one idea per
   sentence, subject – verb – object. If a sentence needs reading twice, split it.
2. **No idioms or coined images.** Not "fly off", "hop round", "for ever", "a hair apart",
   "pulls the dots in", "keeps its secret", "a number for a name", "piles up against". Say
   the plain thing: "keeps growing", "move between", "always", "very close together",
   "come closer and closer to".
3. **Use the real maths, explained.** Write the formula: z → z² + c. Name things by their
   proper name and say once what it means and what it looks like on screen: "the main
   cardioid, the big heart-shaped part of the set (we call it the heart)". A formula with a
   worked example beats a vague picture in words.
4. **Every reference names something visible.** Not "the picture", "here", "the number",
   "the point", "the dots" on their own. Say "the small circle (the marker)", "the white
   dots", "the mini-map at the bottom left", "the panel on the left". The reader must know
   at once where to look.
5. **Say only what the screen shows at that step.** Look at the screenshot of the step. If a
   card says the dots move between four places, the four places must be visible, or the card
   says where to see them ("on the mini-map, the small map at the bottom left"). Do not
   describe motion the reader cannot see.
6. **Numbers worked out, not hinted at.** "With c = −1, z goes 0, −1, 0, −1, …", not "it
   repeats". Ratios: "each bulb is about 4.7 times shorter than the one before", not "the
   ratio tends to 4.669".
7. **One new idea per card.** A card introduces at most one new term. Build on earlier
   cards and on the first tour ("z² + c, step by step"); do not re-explain the formula in
   every tour, but you may remind in one short sentence.
8. **Length.** Clarity first. A card may be longer if that makes it clearer, but cut every
   sentence that adds nothing. Two or three short paragraphs is a good size.
9. **Facts stay exact.** Rewriting for clarity never changes a number, a claim or a link.
   Claims about videos stay attributed to the person who makes them.

## Glossary

Use these words, and only these, for these things. Introduce each the first time a tour uses
it (the first tour introduces all of them).

| Word | Means | On screen |
|---|---|---|
| **the formula** | z → z² + c: square z, add c | |
| **c** | the number being looked at; every point of the picture is one c | |
| **z** | the number the formula changes at each step; it starts at 0 | |
| **step** | one use of the formula | |
| **the marker** | the point c the tour is looking at | a small circle |
| **the dots** | the values of z, one per step (the *orbit*) | white dots joined by lines |
| **labelled point** | a point the tour names with a short text | a text label at a point |
| **in the set** | z stays small for ever (it never gets further than 2 from 0) | black inside, or the interior colour |
| **escapes** | z keeps growing; once it is further than 2 from 0 it always keeps growing | coloured outside |
| **the heart** | the main cardioid, the big heart-shaped part of the set | |
| **bulb** | a round part attached to the heart or to another bulb | |
| **loop of n**, **period n** | the dots end up moving round n places, again and again | |
| **minibrot** | a small copy of the whole set | |
| **the mini-map** | the small map of the whole set | bottom left, can be enlarged |
| **the real line** | the horizontal line through the middle of the set: the ordinary numbers | |

Avoid: "the ring", "test point", "the rule" (say "the formula"), "spot" (say "place"),
"fly off" (say "escape" or "keep growing").

## Examples

| Hard to follow | Clear |
|---|---|
| Points on the middle line have a number for a name. | The horizontal line through the middle holds the ordinary numbers: 0 is inside the heart, −2 is the tip on the far left. |
| The one spot no longer pulls the dots in. | Here the dots no longer come closer to one place. |
| Each dot shows the number that one more step makes. | Each white dot is the value of z after one more step. |
| The bulbs shrink by about 4.669 each and pile up against it. | Each bulb is about 4.669 times shorter than the one before. So the bulbs come closer and closer together, and they end at this point. |

## Process

Write each tour against screenshots of its steps (`--ui-demo=tour-<id>`, see
`Sources/AlmondbreadKit/SelfTest.swift`): every sentence is checked against what the step
shows. Then read the whole tour once as the reader above would.
