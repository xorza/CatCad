# Issues

- `Leaning::ends` (`solid/meeting/seeding/leaning.rs:107`) overruns its
  `Inline<f64, ENDS>` where a leaning drill's turn count changes in more cells
  than the cap holds. Four `catcad` tests panic on it:
  `tests::fields::a_right_click_in_a_form_field_opens_its_menu`,
  `tests::fields::a_form_of_two_fields_lets_the_second_take_the_caret`,
  `tests::fields::dragging_the_turn_arrow_writes_how_much_of_a_turn_is_swept`,
  and
  `tests::editing::picking_a_region_and_a_line_offers_a_revolve_and_the_form_settles_what_it_does`.
- `catcad`'s `build::tests::a_step_the_kernel_will_not_merge_stands_beside_the_model`
  asks for `Built::Refused` where two cylinders of different radii cross on
  crossing axes, and the boolean answers `Built::Made`.
