# Coverage beyond blocks

Suites: `incremental`, `opt`, `threads`, `ui`. Read with `mirth-lab coverage-dims`; arms and configuration arms are in `mirth-lab callgraph`'s report.

## Keyed engine coverage (per query)

1124 functions of the query engine and its per-query plumbing are keyed by the query's dep kind. 330 of 338 dep kinds were seen; 2357 engine blocks were taken by some query; summed over queries, 49333 (query, block) pairs were taken where block coverage counts 2357 blocks.

The queries with the most engine paths other queries took and they never did (top 25):

| query | engine functions entered | engine blocks taken | blocks other queries took there, this one never |
|---|---:|---:|---:|
| `hir_owner_parent_q` | 11 | 123 | 46 |
| `is_mir_available` | 17 | 183 | 45 |
| `normalize_canonicalized_free_alias` | 10 | 118 | 45 |
| `impl_self_is_guaranteed_unsized` | 15 | 159 | 43 |
| `collect_return_position_impl_trait_in_trait_tys` | 15 | 143 | 42 |
| `type_of_opaque` | 13 | 138 | 41 |
| `is_panic_runtime` | 13 | 138 | 41 |
| `associated_types_for_impl_traits_in_trait_or_impl` | 13 | 138 | 41 |
| `assumed_wf_types` | 13 | 138 | 41 |
| `dylib_dependency_formats` | 13 | 138 | 41 |
| `required_panic_strategy` | 13 | 138 | 41 |
| `panic_in_drop_strategy` | 13 | 138 | 41 |
| `symbol_mangling_version` | 13 | 138 | 41 |
| `foreign_modules` | 13 | 138 | 41 |
| `resolve_bound_vars` | 13 | 138 | 41 |
| `type_of_opaque_hir_typeck` | 12 | 133 | 40 |
| `opaque_ty_origin` | 12 | 133 | 40 |
| `nested_bodies_within` | 12 | 133 | 40 |
| `item_bounds` | 12 | 133 | 40 |
| `native_libraries` | 12 | 133 | 40 |
| `lint_expectations` | 12 | 133 | 40 |
| `mir_keys` | 12 | 133 | 40 |
| `closure_saved_names_of_captured_variables` | 12 | 133 | 40 |
| `should_inherit_track_caller` | 12 | 133 | 40 |
| `inherited_align` | 12 | 133 | 40 |

Dep kinds no keyed engine function ran for (8): `Null`, `Red`, `SideEffect`, `AnonZeroDeps`, `TraitSelect`, `CompileCodegenUnit`, `CompileMonoItem`, `Metadata`

Engine paths per query: for each query kind seen, whether it entered each path (✓).

| query | check_feedable_consistency | ensure_can_skip_execution | execute_job_incr | execute_job_non_incr | force_query_dep_node | handle_cycle | load_from_disk_or_invoke_provider_green | wait_for_query |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| `derive_macro_expansion` | | | ✓ | | | | ✓ | |
| `trigger_delayed_bug` | | ✓ | ✓ | | | | | |
| `registered_attr_tools` | | | ✓ | ✓ | | | | |
| `registered_lint_tools` | | | ✓ | ✓ | | | | |
| `early_lint_checks` | | | ✓ | ✓ | | | | |
| `env_var_os` | | | | ✓ | | | | |
| `local_source_files_fingerprint` | | | ✓ | ✓ | ✓ | | | |
| `resolutions` | | | ✓ | ✓ | | | | |
| `resolver_for_lowering_raw` | | | ✓ | ✓ | | ✓ | | |
| `index_ast` | | | ✓ | ✓ | ✓ | | | |
| `source_span` | | | ✓ | ✓ | ✓ | | | |
| `resolve_type_relative_delegations` | | | ✓ | ✓ | | | | |
| `lower_to_hir` | | | ✓ | ✓ | ✓ | | | ✓ |
| `hir_owner` | ✓ | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `hir_crate_items` | | | ✓ | ✓ | | | | |
| `hir_module_items` | | | ✓ | ✓ | ✓ | | ✓ | |
| `hir_owner_parent_q` | | | ✓ | | ✓ | | ✓ | |
| `hir_attr_map` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `const_param_default` | | | | ✓ | | | | ✓ |
| `const_of_item` | | | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `type_of` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `type_of_opaque` | | | ✓ | ✓ | ✓ | | ✓ | |
| `type_of_opaque_hir_typeck` | | | ✓ | ✓ | | | ✓ | |
| `type_alias_is_checked` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `collect_return_position_impl_trait_in_trait_tys` | | | ✓ | ✓ | ✓ | | | |
| `opaque_ty_origin` | | | ✓ | ✓ | | | ✓ | |
| `unsizing_params_for_adt` | | | ✓ | ✓ | | | ✓ | ✓ |
| `analysis` | | ✓ | ✓ | ✓ | | | | |
| `check_expectations` | | ✓ | ✓ | ✓ | | | | |
| `generics_of` | ✓ | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `clauses_of` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `opaque_types_defined_by` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `nested_bodies_within` | | | ✓ | ✓ | | | ✓ | |
| `explicit_item_bounds` | ✓ | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `explicit_item_self_bounds` | ✓ | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `item_bounds` | | | ✓ | ✓ | | | ✓ | |
| `item_self_bounds` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `item_non_self_bounds` | | | | ✓ | | | | ✓ |
| `impl_super_outlives` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `native_libraries` | | | ✓ | ✓ | | | ✓ | |
| `shallow_lint_levels_on` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `lint_expectations` | | | ✓ | ✓ | | | ✓ | |
| `skippable_lints` | | | ✓ | ✓ | ✓ | | | ✓ |
| `expn_that_defined` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_panic_runtime` | | | ✓ | ✓ | ✓ | | ✓ | |
| `check_representability` | | ✓ | ✓ | ✓ | | ✓ | | ✓ |
| `check_representability_adt_ty` | | ✓ | ✓ | ✓ | | ✓ | | ✓ |
| `params_in_repr` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `thir_body` | | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `mir_keys` | | | ✓ | ✓ | | | ✓ | |
| `mir_const_qualif` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `mir_built` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `thir_abstract_const` | | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `mir_drops_elaborated_and_const_checked` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `mir_for_ctfe` | | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `mir_promoted` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `closure_typeinfo` | | | ✓ | ✓ | | | ✓ | ✓ |
| `closure_saved_names_of_captured_variables` | | | ✓ | ✓ | | | ✓ | |
| `mir_coroutine_witnesses` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `check_coroutine_obligations` | | ✓ | ✓ | ✓ | ✓ | | | |
| `check_potentially_region_dependent_goals` | | | | ✓ | | | | |
| `optimized_mir` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_eligible_for_coverage` | | | | ✓ | | | | |
| `coverage_attr_on` | ✓ | | ✓ | ✓ | | | | |
| `coverage_codegen_info` | | | | | | | | |
| `promoted_mir` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `erase_and_anonymize_regions_ty` | | | ✓ | ✓ | | | ✓ | ✓ |
| `wasm_import_module_map` | | | | | | | | |
| `trait_explicit_clauses_and_bounds` | | | ✓ | ✓ | ✓ | | | ✓ |
| `explicit_clauses_of` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `inferred_outlives_of` | ✓ | | ✓ | ✓ | ✓ | | ✓ | |
| `explicit_super_clauses_of` | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `explicit_implied_clauses_of` | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `explicit_supertraits_containing_assoc_item` | | | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `const_conditions` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `explicit_implied_const_bounds` | | | ✓ | ✓ | | | ✓ | ✓ |
| `type_param_clauses` | | | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `trait_def` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `adt_def` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `adt_destructor` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `adt_async_destructor` | | | ✓ | ✓ | | | ✓ | ✓ |
| `adt_sizedness_constraint` | | | ✓ | ✓ | | | ✓ | ✓ |
| `adt_dtorck_constraint` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `constness` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `asyncness` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_promotable_const_fn` | | | ✓ | ✓ | | | ✓ | ✓ |
| `coroutine_by_move_body_def_id` | | | ✓ | ✓ | | | | |
| `coroutine_kind` | ✓ | | ✓ | ✓ | ✓ | | ✓ | |
| `coroutine_for_closure` | | | ✓ | ✓ | | | | |
| `coroutine_hidden_types` | | | ✓ | ✓ | | | ✓ | ✓ |
| `crate_variances` | | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `variances_of` | | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `inferred_outlives_crate` | | | ✓ | ✓ | ✓ | | | ✓ |
| `associated_item_def_ids` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `associated_item` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `associated_items` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `impl_item_implementor_ids` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `associated_types_for_impl_traits_in_trait_or_impl` | | | ✓ | ✓ | ✓ | | ✓ | |
| `impl_trait_header` | | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `impl_is_fully_generic_for_reflection` | | | ✓ | ✓ | | | ✓ | |
| `impl_self_is_guaranteed_unsized` | | | ✓ | ✓ | ✓ | ✓ | ✓ | |
| `inherent_impls` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `incoherent_impls` | | | ✓ | ✓ | | | ✓ | ✓ |
| `check_transmutes` | | ✓ | ✓ | ✓ | | | ✓ | ✓ |
| `check_offloads` | | | | ✓ | | | | |
| `check_unsafety` | | ✓ | ✓ | ✓ | | | | |
| `check_tail_calls` | | ✓ | ✓ | ✓ | ✓ | | | |
| `assumed_wf_types` | | | ✓ | ✓ | ✓ | | ✓ | |
| `assumed_wf_types_for_rpitit` | | | ✓ | ✓ | | | ✓ | ✓ |
| `fn_sig` | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `lint_mod` | | ✓ | ✓ | ✓ | | | | |
| `check_unused_traits` | | ✓ | ✓ | ✓ | | | | |
| `check_mod_attrs` | | ✓ | ✓ | ✓ | | | | |
| `check_mod_unstable_api_usage` | | ✓ | ✓ | ✓ | | | | |
| `check_mod_privacy` | | ✓ | ✓ | ✓ | | | | |
| `check_liveness` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `live_symbols_and_ignored_derived_traits` | | | ✓ | ✓ | ✓ | | | ✓ |
| `check_mod_deathness` | | ✓ | ✓ | ✓ | | | | |
| `check_type_wf` | | ✓ | ✓ | ✓ | | | | |
| `coerce_unsized_info` | | | | ✓ | | | | ✓ |
| `typeck_root` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `used_trait_imports` | | | ✓ | ✓ | ✓ | | ✓ | |
| `coherent_trait` | | ✓ | ✓ | ✓ | ✓ | ✓ | | ✓ |
| `mir_borrowck` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `crate_inherent_impls` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `crate_inherent_impls_validity_check` | | ✓ | ✓ | ✓ | | | | |
| `crate_inherent_impls_overlap_check` | | ✓ | ✓ | ✓ | | | | |
| `orphan_check_impl` | | ✓ | ✓ | ✓ | | | | |
| `mir_callgraph_cyclic` | | | | ✓ | | | | |
| `mir_inliner_callees` | | | ✓ | ✓ | | | | ✓ |
| `tag_for_variant` | | | | ✓ | | | | ✓ |
| `eval_to_allocation_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `eval_static_initializer` | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `eval_to_const_value_raw` | | ✓ | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `eval_to_valtree` | | | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `valtree_to_const_val` | | | ✓ | ✓ | | | ✓ | ✓ |
| `lit_to_const` | | | ✓ | ✓ | | | ✓ | ✓ |
| `check_match` | | ✓ | ✓ | ✓ | ✓ | | | |
| `effective_visibilities` | | ✓ | ✓ | ✓ | ✓ | | | ✓ |
| `check_private_in_public` | | ✓ | ✓ | ✓ | | | | |
| `reachable_set` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `region_scope_tree` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `mir_shims` | | | ✓ | ✓ | | | ✓ | ✓ |
| `symbol_name` | | | ✓ | ✓ | | | ✓ | ✓ |
| `def_kind` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `def_span` | ✓ | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `def_ident_span` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `ty_span` | | | ✓ | ✓ | ✓ | | ✓ | |
| `lookup_stability` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `lookup_const_stability` | | | ✓ | ✓ | | | | ✓ |
| `lookup_default_body_stability` | | | ✓ | ✓ | | | | ✓ |
| `should_inherit_track_caller` | | | ✓ | ✓ | | | ✓ | |
| `inherited_align` | | | ✓ | ✓ | | | ✓ | |
| `lookup_deprecation_entry` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_doc_hidden` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_doc_notable_trait` | | | | | | | | |
| `attrs_for_def` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `codegen_fn_attrs` | ✓ | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `asm_target_features` | | | ✓ | ✓ | | | ✓ | |
| `fn_arg_idents` | | | ✓ | ✓ | | | ✓ | |
| `rendered_const` | | | | | | | | |
| `rendered_precise_capturing_args` | | | ✓ | ✓ | | | | |
| `impl_parent` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_mir_available` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `own_existential_vtable_entries` | | | ✓ | ✓ | | | ✓ | ✓ |
| `vtable_entries` | | | ✓ | ✓ | | | ✓ | ✓ |
| `first_method_vtable_slot` | | | ✓ | ✓ | | | | ✓ |
| `supertrait_vtable_slot` | | | | ✓ | | | | |
| `vtable_allocation` | | | ✓ | ✓ | | | ✓ | |
| `codegen_select_candidate` | | | ✓ | ✓ | | | ✓ | ✓ |
| `all_local_trait_impls` | | | ✓ | ✓ | | | ✓ | |
| `local_trait_impls` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `trait_impls_of` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `specialization_graph_of` | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `dyn_compatibility_violations` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_dyn_compatible` | | | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `param_env` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `param_env_normalized_for_post_analysis` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_copy_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_use_cloned_raw` | | | | | | | | |
| `is_sized_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_freeze_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_unsafe_unpin_raw` | | | ✓ | ✓ | | | ✓ | |
| `is_unpin_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_async_drop_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `needs_drop_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `needs_async_drop_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `has_significant_drop_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `has_structural_eq_impl` | | | ✓ | ✓ | | | | |
| `adt_drop_tys` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `adt_async_drop_tys` | | | | | | | | |
| `adt_significant_drop_tys` | | | | | | | | |
| `list_significant_drop_tys` | | | | ✓ | | | | |
| `layout_of` | | ✓ | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `fn_abi_of_fn_ptr` | | | ✓ | ✓ | | | ✓ | ✓ |
| `fn_abi_of_instance_no_deduced_attrs` | | | ✓ | ✓ | | | ✓ | ✓ |
| `fn_abi_of_instance_raw` | | | | ✓ | | | | |
| `dylib_dependency_formats` | | | ✓ | ✓ | ✓ | | ✓ | |
| `dependency_formats` | | | ✓ | ✓ | | | ✓ | |
| `is_compiler_builtins` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `has_global_allocator` | | | ✓ | ✓ | ✓ | | | |
| `has_alloc_error_handler` | | | ✓ | ✓ | ✓ | | | |
| `has_panic_handler` | | | ✓ | ✓ | | | ✓ | |
| `is_profiler_runtime` | | | ✓ | ✓ | | | ✓ | |
| `has_ffi_unwind_calls` | | ✓ | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `required_panic_strategy` | | | ✓ | ✓ | ✓ | | ✓ | |
| `panic_in_drop_strategy` | | | ✓ | ✓ | ✓ | | ✓ | |
| `is_no_builtins` | | | ✓ | ✓ | | | ✓ | |
| `symbol_mangling_version` | | | ✓ | ✓ | ✓ | | ✓ | |
| `extern_crate` | | | ✓ | ✓ | ✓ | | | |
| `specialization_enabled_in` | | | ✓ | ✓ | | | ✓ | ✓ |
| `specializes` | | | ✓ | ✓ | | ✓ | ✓ | ✓ |
| `defaultness` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `default_field` | | | | | | | | |
| `check_well_formed` | | ✓ | ✓ | ✓ | ✓ | | | |
| `enforce_impl_non_lifetime_params_are_constrained` | | ✓ | ✓ | ✓ | | | | ✓ |
| `reachable_non_generics` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `is_reachable_non_generic` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_unreachable_local_definition` | | | ✓ | ✓ | | | | |
| `upstream_monomorphizations` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `upstream_monomorphizations_for` | | | ✓ | ✓ | | | ✓ | ✓ |
| `upstream_drop_glue_for` | | | | ✓ | | | | ✓ |
| `upstream_async_drop_glue_for` | | | | ✓ | | | | |
| `foreign_modules` | | | ✓ | ✓ | ✓ | | ✓ | |
| `clashing_extern_declarations` | | ✓ | ✓ | ✓ | | | | |
| `entry_fn` | | ✓ | ✓ | ✓ | | | ✓ | |
| `proc_macro_decls_static` | | ✓ | ✓ | ✓ | | | ✓ | |
| `crate_hash` | | ✓ | ✓ | ✓ | ✓ | | | |
| `crate_host_hash` | | | ✓ | ✓ | ✓ | | | |
| `extra_filename` | | | ✓ | ✓ | ✓ | | | |
| `crate_extern_paths` | | | | ✓ | | | | |
| `implementations_of_trait` | | | ✓ | ✓ | | | ✓ | |
| `crate_incoherent_impls` | | | ✓ | ✓ | | | ✓ | |
| `native_library` | | | ✓ | ✓ | | | | |
| `inherit_sig_for_delegation_item` | | | ✓ | ✓ | | | | |
| `delegation_user_specified_args` | | | ✓ | ✓ | | | | |
| `resolve_bound_vars` | | | ✓ | ✓ | ✓ | | ✓ | |
| `named_variable_map` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_late_bound_map` | | | ✓ | ✓ | | | ✓ | |
| `object_lifetime_default` | | | ✓ | ✓ | | | ✓ | |
| `late_bound_vars_map` | | | ✓ | ✓ | | | ✓ | |
| `opaque_captured_lifetimes` | | | ✓ | ✓ | | | ✓ | |
| `live_args_for_alias_from_outlives_bounds` | | | ✓ | ✓ | | | ✓ | |
| `args_known_to_outlive_alias_params` | | | ✓ | ✓ | | | ✓ | |
| `visibility` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `inhabited_predicate_for_def` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `inhabited_predicate_type` | | | ✓ | ✓ | | | ✓ | ✓ |
| `is_opsem_inhabited_adt_cached` | | | ✓ | ✓ | | | ✓ | ✓ |
| `crate_dep_kind` | | | ✓ | ✓ | ✓ | | | |
| `crate_name` | ✓ | | ✓ | ✓ | | | ✓ | |
| `module_children` | | | ✓ | ✓ | | | ✓ | |
| `num_extern_def_ids` | | | | | | | | |
| `lib_features` | | | ✓ | ✓ | | | ✓ | |
| `stability_implications` | | | ✓ | ✓ | | | ✓ | |
| `intrinsic_raw` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `get_lang_items` | | | ✓ | ✓ | | | | |
| `all_diagnostic_items` | | | ✓ | ✓ | ✓ | | | ✓ |
| `all_canonical_symbols` | | | ✓ | ✓ | ✓ | | | ✓ |
| `defined_lang_items` | | | ✓ | ✓ | | | ✓ | |
| `diagnostic_items` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `canonical_symbols` | | | ✓ | ✓ | | | ✓ | |
| `missing_lang_items` | | | ✓ | ✓ | | | ✓ | |
| `visible_parent_map` | | | ✓ | ✓ | | | ✓ | ✓ |
| `trimmed_def_paths` | | | ✓ | ✓ | ✓ | | | ✓ |
| `missing_extern_crate_item` | | | ✓ | ✓ | ✓ | | | |
| `used_crate_source` | | | ✓ | ✓ | ✓ | | | |
| `debugger_visualizers` | | | ✓ | ✓ | ✓ | | | |
| `postorder_cnums` | | | ✓ | ✓ | | | | |
| `is_private_dep` | | | ✓ | ✓ | ✓ | | | ✓ |
| `allocator_kind` | | | ✓ | ✓ | | | | |
| `alloc_error_handler_kind` | | | ✓ | ✓ | | | | |
| `upvars_mentioned` | | | ✓ | ✓ | | | ✓ | |
| `crates` | | | ✓ | ✓ | ✓ | | | |
| `used_crates` | | | ✓ | ✓ | | | | |
| `duplicate_crate_names` | | | | ✓ | | | | |
| `traits` | | | ✓ | ✓ | | | ✓ | |
| `trait_impls_in_crate` | | | | | | | | |
| `stable_order_of_exportable_impls` | | ✓ | ✓ | ✓ | ✓ | | ✓ | |
| `exportable_items` | | ✓ | ✓ | ✓ | ✓ | | ✓ | |
| `exported_non_generic_symbols` | | | ✓ | ✓ | | | ✓ | |
| `exported_generic_symbols` | | | ✓ | ✓ | ✓ | | ✓ | |
| `collect_and_partition_mono_items` | | | ✓ | ✓ | ✓ | | | |
| `is_codegened_item` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `codegen_unit` | | ✓ | ✓ | ✓ | | | ✓ | |
| `backend_optimization_level` | | | ✓ | ✓ | | | ✓ | |
| `output_filenames` | | | | | | | | |
| `normalize_canonicalized_projection` | | | ✓ | ✓ | | | ✓ | ✓ |
| `normalize_canonicalized_free_alias` | | | ✓ | | | | ✓ | |
| `normalize_canonicalized_inherent_projection` | | | | ✓ | | | | |
| `try_normalize_generic_arg_after_erasing_regions` | | | ✓ | ✓ | | | ✓ | ✓ |
| `implied_outlives_bounds` | | | ✓ | ✓ | | | ✓ | ✓ |
| `mir_borrowck_implied_outlives_bounds` | | | ✓ | ✓ | | | | |
| `dropck_outlives` | | | ✓ | ✓ | | | ✓ | ✓ |
| `evaluate_obligation` | | | ✓ | ✓ | | | ✓ | ✓ |
| `type_op_ascribe_user_type` | | | ✓ | ✓ | | | ✓ | ✓ |
| `type_op_prove_predicate` | | | ✓ | ✓ | | | ✓ | ✓ |
| `type_op_normalize_ty` | | | ✓ | ✓ | | | ✓ | ✓ |
| `type_op_normalize_clause` | | | ✓ | ✓ | | | ✓ | ✓ |
| `type_op_normalize_poly_fn_sig` | | | | | | | | |
| `type_op_normalize_fn_sig` | | | ✓ | ✓ | | | ✓ | ✓ |
| `instantiate_and_check_impossible_clauses` | | | ✓ | ✓ | | | ✓ | |
| `is_impossible_associated_item` | | | | | | | | |
| `method_autoderef_steps` | | | ✓ | ✓ | | | ✓ | ✓ |
| `evaluate_root_goal_for_proof_tree_raw` | | | ✓ | ✓ | | | | ✓ |
| `all_rust_target_features` | | | ✓ | ✓ | ✓ | | | ✓ |
| `implied_target_features` | | | | ✓ | | | | ✓ |
| `features_query` | | | | | | | | |
| `crate_for_resolver` | | | | | | | | |
| `resolve_instance_raw` | | | ✓ | ✓ | | | ✓ | ✓ |
| `reveal_opaque_types_in_bounds` | | | ✓ | ✓ | | | ✓ | ✓ |
| `limits` | | ✓ | ✓ | ✓ | | | ✓ | |
| `diagnostic_hir_wf_check` | | | ✓ | ✓ | | | | |
| `check_validity_requirement` | | | ✓ | ✓ | | | | |
| `compare_impl_item` | | ✓ | ✓ | ✓ | ✓ | | | |
| `deduced_param_attrs` | | | | ✓ | | | | |
| `doc_link_resolutions` | | | | | | | | |
| `doc_link_traits_in_scope` | | | | | | | | |
| `stripped_cfg_items` | | | ✓ | ✓ | | | ✓ | |
| `generics_require_sized_self` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `cross_crate_inlinable` | | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `check_mono_item` | | ✓ | ✓ | ✓ | | | | ✓ |
| `items_of_instance` | | | ✓ | ✓ | | | ✓ | |
| `size_estimate` | | | ✓ | ✓ | | | ✓ | |
| `anon_const_kind` | | | ✓ | ✓ | | | ✓ | ✓ |
| `trivial_const` | | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `sanitizer_settings_for` | ✓ | | ✓ | ✓ | ✓ | | ✓ | ✓ |
| `check_externally_implementable_items` | | ✓ | ✓ | ✓ | | | | |
| `externally_implementable_items` | | | ✓ | ✓ | ✓ | | ✓ | |
| `fake_doc_items` | | | ✓ | ✓ | | | ✓ | |
| `all_fake_doc_items` | | | | | | | | |
| **queries** | 23 | 56 | 293 | 308 | 133 | 28 | 210 | 166 |

## Incremental transitions (per dep-node kind)

25 dependency-graph functions keyed by the node's kind; 302 kinds seen. Cells: blocks taken for that kind / blocks of the function.

| dep kind | try_force_from_dep_node | extract_def_id | alloc_new_node | assert_dep_node_not_yet_allocated_in_current_session | debug_dep_kind_was_loaded_from_disk::{closure#0} | debug_was_loaded_from_disk | is_green | is_red | node_color | try_mark_green | with_feed_task | with_task | alloc_and_color_node | assert_dep_node_not_yet_allocated_in_current_session | hash_result_and_alloc_node | mark_debug_loaded_from_disk | node_color | try_mark_green | with_task | push | record | send_and_color | send_new | node_to_index_opt | new |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| `Red` | | | 2/2 | | | | | | | | | | | | | | | | | 9/12 | 7/9 | | 12/12 | | 20/20 |
| `SideEffect` | 10/50 | | | | | | | | | | | | | | | | | | | 12/12 | 7/9 | 17/24 | 12/12 | | 20/20 |
| `AnonZeroDeps` | | | 2/2 | | | | | | | | | | | | | | | | | 9/12 | 7/9 | | 12/12 | | 20/20 |
| `TraitSelect` | 8/50 | | 2/2 | | | | | | | | | | | | | | | | | 12/12 | 7/9 | | 12/12 | | 20/20 |
| `CompileCodegenUnit` | | | 2/2 | 5/6 | | | | | | 6/8 | | 8/8 | 15/16 | 10/17 | 7/7 | | | 15/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `Metadata` | | | 2/2 | | | | | | | 6/8 | | 8/8 | 12/16 | 10/17 | 7/7 | | | 15/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 19/20 |
| `derive_macro_expansion` | | | 2/2 | | 2/2 | | | | | | | | 7/16 | 6/17 | 7/7 | 5/5 | | 17/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `trigger_delayed_bug` | | | 2/2 | | | | | | | 6/8 | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `registered_attr_tools` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `registered_lint_tools` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `early_lint_checks` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `local_source_files_fingerprint` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `resolutions` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `resolver_for_lowering_raw` | | | 2/2 | | | | | | | | | | 12/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `index_ast` | 10/50 | | 2/2 | | | | | | | | | | 12/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `source_span` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `resolve_type_relative_delegations` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `lower_to_hir` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `hir_owner` | 10/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | | 20/23 | | 15/16 | 10/17 | 7/7 | | 7/7 | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `hir_crate_items` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `hir_module_items` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `hir_owner_parent_q` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `hir_attr_map` | 10/50 | 7/8 | 2/2 | | | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `const_of_item` | | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `type_of` | 14/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | 6/8 | 23/23 | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `type_of_opaque` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `type_of_opaque_hir_typeck` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `type_alias_is_checked` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `collect_return_position_impl_trait_in_trait_tys` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | | 5/9 | 17/24 | 12/12 | 14/14 | 19/20 |
| `opaque_ty_origin` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `unsizing_params_for_adt` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `analysis` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_expectations` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `generics_of` | 10/50 | 7/8 | 2/2 | | 2/2 | | 2/2 | 2/2 | 5/6 | 6/8 | 20/23 | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `clauses_of` | 10/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | 6/8 | | | 15/16 | 10/17 | 7/7 | | 7/7 | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `opaque_types_defined_by` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `nested_bodies_within` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `explicit_item_bounds` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `explicit_item_self_bounds` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `item_bounds` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `item_self_bounds` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `impl_super_outlives` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `native_libraries` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `shallow_lint_levels_on` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `lint_expectations` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `skippable_lints` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `expn_that_defined` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `is_panic_runtime` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_representability` | 8/50 | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_representability_adt_ty` | 8/50 | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `params_in_repr` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 12/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `thir_body` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 12/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `mir_keys` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `mir_const_qualif` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `mir_built` | 10/50 | 7/8 | 2/2 | | | | | | | | 16/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `thir_abstract_const` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `mir_drops_elaborated_and_const_checked` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 12/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `mir_for_ctfe` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `mir_promoted` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 12/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `closure_typeinfo` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `closure_saved_names_of_captured_variables` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `mir_coroutine_witnesses` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | | 5/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_coroutine_obligations` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `optimized_mir` | 10/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | | | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `coverage_attr_on` | | | 2/2 | | | | | | | | 16/23 | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `promoted_mir` | 10/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | | | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `erase_and_anonymize_regions_ty` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `trait_explicit_clauses_and_bounds` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `explicit_clauses_of` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | | 16/23 | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `inferred_outlives_of` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `explicit_super_clauses_of` | 14/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `explicit_implied_clauses_of` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `explicit_supertraits_containing_assoc_item` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `const_conditions` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `explicit_implied_const_bounds` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `type_param_clauses` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | | 5/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `trait_def` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `adt_def` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `adt_destructor` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `adt_async_destructor` | | 7/8 | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `adt_sizedness_constraint` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `adt_dtorck_constraint` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `constness` | 10/50 | 7/8 | 2/2 | | | | | | | | 16/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `asyncness` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_promotable_const_fn` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `coroutine_by_move_body_def_id` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 24/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `coroutine_kind` | 10/50 | 7/8 | 2/2 | | | | | | | | 16/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `coroutine_for_closure` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `coroutine_hidden_types` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `crate_variances` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `variances_of` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `inferred_outlives_crate` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `associated_item_def_ids` | 10/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | | | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `associated_item` | 10/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | | 20/23 | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `associated_items` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `impl_item_implementor_ids` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `associated_types_for_impl_traits_in_trait_or_impl` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `impl_trait_header` | 10/50 | 7/8 | 2/2 | | | | 2/2 | 2/2 | 5/6 | | | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `impl_is_fully_generic_for_reflection` | | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 17/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `impl_self_is_guaranteed_unsized` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `inherent_impls` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `incoherent_impls` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_transmutes` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `check_unsafety` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_tail_calls` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `assumed_wf_types` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `assumed_wf_types_for_rpitit` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 11/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `fn_sig` | 10/50 | 7/8 | 2/2 | | 2/2 | | 2/2 | 2/2 | 5/6 | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `lint_mod` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_unused_traits` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_mod_attrs` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_mod_unstable_api_usage` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_mod_privacy` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_liveness` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `live_symbols_and_ignored_derived_traits` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `check_mod_deathness` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_type_wf` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `typeck_root` | 14/50 | 7/8 | 2/2 | | | 8/8 | 2/2 | 2/2 | 5/6 | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | 7/7 | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `used_trait_imports` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `coherent_trait` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `mir_borrowck` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `crate_inherent_impls` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `crate_inherent_impls_validity_check` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `crate_inherent_impls_overlap_check` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `orphan_check_impl` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `mir_inliner_callees` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 8/14 | 20/20 |
| `eval_to_allocation_raw` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `eval_static_initializer` | | 7/8 | 2/2 | | | | | | | 6/8 | 4/23 | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `eval_to_const_value_raw` | 8/50 | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `eval_to_valtree` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `valtree_to_const_val` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 11/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `lit_to_const` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | 9/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `check_match` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `effective_visibilities` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `check_private_in_public` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `reachable_set` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `region_scope_tree` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `mir_shims` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `symbol_name` | 8/50 | | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `def_kind` | 10/50 | 7/8 | 2/2 | | | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `def_span` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `def_ident_span` | 10/50 | 7/8 | 2/2 | | | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `ty_span` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `lookup_stability` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `lookup_const_stability` | | 7/8 | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 8/14 | 20/20 |
| `lookup_default_body_stability` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 8/14 | 20/20 |
| `should_inherit_track_caller` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `inherited_align` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `lookup_deprecation_entry` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_doc_hidden` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `attrs_for_def` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `codegen_fn_attrs` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | 6/8 | 16/23 | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `asm_target_features` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `fn_arg_idents` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `rendered_precise_capturing_args` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `impl_parent` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `is_mir_available` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `own_existential_vtable_entries` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `vtable_entries` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `first_method_vtable_slot` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `vtable_allocation` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `codegen_select_candidate` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `all_local_trait_impls` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `local_trait_impls` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `trait_impls_of` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `specialization_graph_of` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `dyn_compatibility_violations` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `is_dyn_compatible` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `param_env` | 10/50 | 7/8 | 2/2 | | | | | | | | 16/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `param_env_normalized_for_post_analysis` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_copy_raw` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `is_sized_raw` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `is_freeze_raw` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_unsafe_unpin_raw` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_unpin_raw` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_async_drop_raw` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `needs_drop_raw` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `needs_async_drop_raw` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `has_significant_drop_raw` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `has_structural_eq_impl` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `adt_drop_tys` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `layout_of` | 8/50 | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `fn_abi_of_fn_ptr` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `fn_abi_of_instance_no_deduced_attrs` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `dylib_dependency_formats` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `dependency_formats` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `is_compiler_builtins` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `has_global_allocator` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `has_alloc_error_handler` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `has_panic_handler` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_profiler_runtime` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `has_ffi_unwind_calls` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `required_panic_strategy` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `panic_in_drop_strategy` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_no_builtins` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `symbol_mangling_version` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `extern_crate` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `specialization_enabled_in` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `specializes` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `defaultness` | 10/50 | 7/8 | 2/2 | | | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `check_well_formed` | 14/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `enforce_impl_non_lifetime_params_are_constrained` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `reachable_non_generics` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `is_reachable_non_generic` | | 7/8 | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `is_unreachable_local_definition` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 8/14 | 20/20 |
| `upstream_monomorphizations` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 19/20 |
| `upstream_monomorphizations_for` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `foreign_modules` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `clashing_extern_declarations` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `entry_fn` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `proc_macro_decls_static` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `crate_hash` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `crate_host_hash` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `extra_filename` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `implementations_of_trait` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `crate_incoherent_impls` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 11/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `native_library` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `inherit_sig_for_delegation_item` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `delegation_user_specified_args` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `resolve_bound_vars` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `named_variable_map` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `is_late_bound_map` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `object_lifetime_default` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `late_bound_vars_map` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `opaque_captured_lifetimes` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `live_args_for_alias_from_outlives_bounds` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `args_known_to_outlive_alias_params` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `visibility` | 10/50 | 7/8 | 2/2 | | | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `inhabited_predicate_for_def` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `inhabited_predicate_type` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `is_opsem_inhabited_adt_cached` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `crate_dep_kind` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `crate_name` | | | 2/2 | | | | | | | | 20/23 | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `module_children` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `lib_features` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `stability_implications` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `intrinsic_raw` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `get_lang_items` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `all_diagnostic_items` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `all_canonical_symbols` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `defined_lang_items` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `diagnostic_items` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `canonical_symbols` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `missing_lang_items` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `visible_parent_map` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 11/19 | 24/32 | | 5/9 | | 12/12 | 14/14 | 19/20 |
| `trimmed_def_paths` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 19/20 |
| `missing_extern_crate_item` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | | 5/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `used_crate_source` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `debugger_visualizers` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `postorder_cnums` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `is_private_dep` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `allocator_kind` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `alloc_error_handler_kind` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `upvars_mentioned` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `crates` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `used_crates` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `traits` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `stable_order_of_exportable_impls` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `exportable_items` | 10/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `exported_non_generic_symbols` | | 7/8 | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `exported_generic_symbols` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `collect_and_partition_mono_items` | 10/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `is_codegened_item` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `codegen_unit` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `backend_optimization_level` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | 9/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `output_filenames` | | | 2/2 | | | | | | | | 20/23 | | 15/16 | | 7/7 | | | | | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `normalize_canonicalized_projection` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `normalize_canonicalized_free_alias` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 17/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `try_normalize_generic_arg_after_erasing_regions` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `implied_outlives_bounds` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `mir_borrowck_implied_outlives_bounds` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | | 5/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `dropck_outlives` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `evaluate_obligation` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `type_op_ascribe_user_type` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `type_op_prove_predicate` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `type_op_normalize_ty` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `type_op_normalize_clause` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `type_op_normalize_fn_sig` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `instantiate_and_check_impossible_clauses` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `method_autoderef_steps` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `evaluate_root_goal_for_proof_tree_raw` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 24/32 | | 5/9 | | 12/12 | 14/14 | 20/20 |
| `all_rust_target_features` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | | 12/32 | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `features_query` | | | 2/2 | | | | | | | | 20/23 | | 15/16 | | 7/7 | | | | | 9/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `crate_for_resolver` | | | 2/2 | | | | | | | | 20/23 | | 12/16 | | 7/7 | | | | | 9/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `resolve_instance_raw` | 8/50 | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `reveal_opaque_types_in_bounds` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 18/19 | 22/32 | 9/12 | 7/9 | | 12/12 | 14/14 | 20/20 |
| `limits` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `diagnostic_hir_wf_check` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | | 12/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `check_validity_requirement` | | | 2/2 | | | | | | | | | | 7/16 | 6/17 | 7/7 | | | 7/19 | 22/32 | | 5/9 | | 12/12 | 8/14 | 20/20 |
| `compare_impl_item` | 14/50 | 7/8 | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `stripped_cfg_items` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `generics_require_sized_self` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `cross_crate_inlinable` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `check_mono_item` | 8/50 | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `items_of_instance` | | | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 17/19 | 24/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `size_estimate` | | | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 17/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `anon_const_kind` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `trivial_const` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 18/24 | 12/12 | 14/14 | 20/20 |
| `sanitizer_settings_for` | 10/50 | 7/8 | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `check_externally_implementable_items` | | | 2/2 | | | | | | | 6/8 | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `externally_implementable_items` | 10/50 | 7/8 | 2/2 | | 2/2 | | | | | | | | 15/16 | 10/17 | 7/7 | 5/5 | | 18/19 | 22/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |
| `fake_doc_items` | | | 2/2 | | | | | | | | | | 15/16 | 10/17 | 7/7 | | | 17/19 | 24/32 | 12/12 | 7/9 | 17/24 | 12/12 | 14/14 | 20/20 |

## Feature gates consulted

Each `Features::<feature>()` accessor, keyed by what it returned. 268 accessors: consulted while on and off 136, only while on 9, only while off 6, never consulted 117.

**Consulted only while off** (code that checks the gate, reached, with the gate never on: what gate-mutate should turn on): `asm_experimental_reg`, `cfg_contract_checks`, `cfg_sanitizer_cfi`, `freeze_impls`, `ref_pat_eat_one_layer_2024_structural`, `rustc_private`

**Consulted only while on**: `cfg_target_thread_local`, `diagnostic_on_const`, `impl_restriction`, `mut_restriction`, `negative_bounds`, `pattern_types`, `static_align`, `unsafe_fields`, `view_types`

**Never consulted**: `aarch64_unstable_target_feature`, `aarch64_ver_target_feature`, `abi_avr_interrupt`, `abi_cmse_nonsecure_call`, `abi_gpu_kernel`, `abi_msp430_interrupt`, `abi_ptx`, `abi_riscv_interrupt`, `abi_swift`, `abi_vectorcall`, `abi_x86_interrupt`, `alloc_error_handler`, `allocator_internals`, `allow_internal_unsafe`, `allow_internal_unstable`, `apx_target_feature`, `arm_target_feature`, `asm_experimental_arch`, `async_fn_in_dyn_trait`, `avr_target_feature`, `avx10_target_feature`, `bpf_target_feature`, `c_variadic_experimental_arch`, `cfi_encoding`, `clflushopt_target_feature`, `cmse_nonsecure_entry`, `compiler_builtins`, `const_async_blocks`, `const_c_variadic`, `const_destruct`, `const_for`, `const_try`, `contracts`, `coverage_attribute`, `csky_target_feature`, `custom_mir`, `custom_test_frameworks`, `derive_from`, `doc_cfg`, `doc_masked`, `doc_notable_trait`, `dropck_eyepatch`, `dump_feature_usage_metrics`, `serialize`, `serialize`, `serialize`, `dump_feature_usage_metrics::{closure#0}`, `effective_target_features`, `eii_internals`, `ermsb_target_feature`, `explicit_extern_abis`, `extern_item_impls`, `f16b`, `ffi_const`, `ffi_pure`, `field_projections`, `field_representing_type_raw`, `fma4_target_feature`, `fn_static`, `forced_keywords`, `fundamental`, `global_registration`, `hexagon_target_feature`, `instrument_fn`, `intra_doc_pointers`, `lahfsahf_target_feature`, `lang_items`, `large_assignments`, `linkage`, `loongarch_target_feature`, `loop_hints`, `loop_match`, `m68k_target_feature`, `marker_trait_attr`, `mips_target_feature`, `movdir64b_target_feature`, `movdiri_target_feature`, `movrs_target_feature`, `multiple_supertrait_upcastable`, `must_not_suspend`, `needs_panic_runtime`, `no_core`, `non_exhaustive_omitted_patterns_lint`, `nvptx_target_feature`, `optimize_attribute`, `panic_runtime`, `patchable_function_entry`, `pattern_complexity_limit`, `powerpc_target_feature`, `prelude_import`, `prfchw_target_feature`, `profiler_runtime`, `register_tool`, `riscv_target_feature`, `rtm_target_feature`, `rust_cold_cc`, `rust_preserve_none_cc`, `rust_tail_cc`, `rustdoc_internals`, `rustdoc_missing_doc_code_examples`, `s390x_target_feature`, `sanitize`, `sparc_target_feature`, `splat`, `strict_provenance_lints`, `structural_match`, `test_binder_constraints`, `test_incomplete_feature`, `test_unstable_lint`, `thread_local`, `unqualified_local_imports`, `unsized_const_params`, `wasm_target_feature`, `x86_amx_intrinsics`, `x87_target_feature`, `xop_target_feature`, `xtensa_target_feature`

## Type kinds

90 functions keyed by the `TyKind` of their first `Ty` argument; 81 ran; 29 of 29 kinds reached at least one.

Kinds that reached none of them: 

| function | kinds at entry | which |
|---|---:|---|
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::peel_refs` | 28 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Bound Placeholder Infer Error |
| `rustc_hir_typeck::coercion::<impl fn_ctxt::FnCtxt<'a, 'tcx>>::coerce` | 26 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::Coerce::<'f, 'tcx>::coerce` | 26 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::Coerce::<'f, 'tcx>::unify` | 26 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::Coerce::<'f, 'tcx>::unify_raw` | 26 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::CoerceMany::<'tcx>::coerce_inner` | 26 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::success` | 26 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::Coerce::<'f, 'tcx>::coerce_unsized` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::Coerce::<'f, 'tcx>::maybe_to_pin_ref` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::CoerceMany::<'tcx>::coerce` | 25 | Bool Char Int Uint Float Adt Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_middle::ty::layout::LayoutOf::layout_of` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::layout::LayoutOf::spanned_layout_of` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::is_sized` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::util::needs_drop_components` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::util::needs_drop_components_with_async` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_ty_utils::layout::layout_of_uncached` | 25 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_hir_typeck::coercion::CoerceMany::<'tcx>::new` | 24 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnPtr UnsafeBinder Dynamic Closure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::CoerceMany::<'tcx>::with_capacity` | 24 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnPtr UnsafeBinder Dynamic Closure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::has_unsafe_fields` | 24 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine CoroutineWitness Never Tuple Alias |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::is_trivially_freeze` | 24 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::needs_drop` | 24 | Bool Char Int Uint Float Adt Str Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::util::<impl ty::context::TyCtxt<'tcx>>::struct_tail_raw` | 24 | Bool Char Int Uint Float Adt Foreign Str Array Pat Slice RawPtr Ref FnDef FnPtr Dynamic Closure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::<impl fn_ctxt::FnCtxt<'a, 'tcx>>::may_coerce` | 23 | Bool Char Int Uint Float Adt Str Array Pat Slice RawPtr Ref FnDef FnPtr Dynamic Closure Coroutine Never Tuple Alias Param Infer Error |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::is_freeze` | 22 | Bool Char Int Uint Float Adt Str Array Slice RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::util::<impl ty::context::TyCtxt<'tcx>>::type_is_copy_modulo_regions` | 22 | Bool Char Int Uint Float Adt Str Array Pat Slice RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param |
| `rustc_middle::ty::util::needs_drop_components_with_async::{closure#0}` | 22 | Bool Char Int Uint Float Adt Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure Coroutine Never Tuple Alias Param Error |
| `rustc_hir_typeck::coercion::<impl fn_ctxt::FnCtxt<'a, 'tcx>>::deref_steps_for_suggestion` | 21 | Bool Char Int Uint Float Adt Str Array RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Never Tuple Alias Param Infer |
| `rustc_ty_utils::layout::layout_of_uncached::{closure#3}::{closure#0}` | 21 | Bool Char Int Uint Float Adt Array Pat Slice RawPtr Ref FnDef FnPtr UnsafeBinder Dynamic Closure Coroutine Never Tuple Alias Param |
| `rustc_middle::ty::util::<impl ty::context::TyCtxt<'tcx>>::type_has_metadata` | 20 | Bool Char Int Uint Float Adt Str Array Pat Slice RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Never Tuple |
| `rustc_ty_utils::abi::fn_abi_new_uncached::{closure#0}` | 20 | Bool Char Int Uint Float Adt Str Array Pat Slice RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Never Tuple |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::has_significant_drop` | 19 | Bool Char Int Uint Float Adt Array RawPtr Ref FnDef FnPtr Closure CoroutineClosure Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::is_trivially_unpin` | 19 | Bool Char Int Uint Float Adt Str Array Slice RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Tuple Param |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::is_unpin` | 19 | Bool Char Int Uint Float Adt Str Array Slice RawPtr Ref FnDef FnPtr Dynamic Closure CoroutineClosure Coroutine Tuple Param |
| `rustc_hir_typeck::coercion::Coerce::<'f, 'tcx>::coerce_to_ref` | 18 | Bool Char Int Uint Float Adt Str Array Slice RawPtr Ref FnDef Closure Tuple Alias Param Infer Error |
| `rustc_hir_typeck::coercion::Coerce::<'f, 'tcx>::maybe_pin_ref_to_ref` | 18 | Bool Char Int Uint Float Adt Str Array Slice RawPtr Ref FnDef Closure Tuple Alias Param Infer Error |
| `rustc_middle::ty::layout::SizeSkeleton::<'tcx>::compute` | 18 | Bool Char Int Uint Float Adt Array Pat RawPtr Ref FnDef FnPtr Coroutine Never Tuple Alias Param Error |
| `rustc_middle::ty::layout::SizeSkeleton::<'tcx>::compute_inner` | 18 | Bool Char Int Uint Float Adt Array Pat RawPtr Ref FnDef FnPtr Coroutine Never Tuple Alias Param Error |
| `rustc_hir_typeck::coercion::CoerceMany::<'tcx>::report_return_mismatched_types` | 17 | Bool Int Uint Float Adt Str Array Pat RawPtr Ref FnPtr Dynamic Never Tuple Alias Param Infer |
| `rustc_middle::ty::util::<impl ty::Ty<'tcx>>::is_unsafe_unpin` | 17 | Bool Char Int Uint Float Adt Str Array Slice RawPtr Ref FnDef FnPtr Closure CoroutineClosure Coroutine Tuple |
| `rustc_hir_typeck::coercion::<impl fn_ctxt::FnCtxt<'a, 'tcx>>::deref_once_mutably_for_diagnostic` | 16 | Bool Int Uint Float Adt Array Pat RawPtr Ref FnDef Dynamic Closure Tuple Param Infer Error |

## Call pairs

162817 distinct (caller, callee) pairs of instrumented functions (156233 with both named; a caller of 0 is a thread's first function).

Of the call graph's 122984 direct and resolved edges between functions that both ran, 116210 were taken as call pairs (94.5%). 40006 pairs are not such edges (calls through function pointers, `dyn`, closures passed through code outside the compiler, or the caller left stale by unwinding).

| callee crate (200+ edges) | edges between functions that ran | taken | % |
|---|---:|---:|---:|
| rustc_arena | 201 | 139 | 69.2 |
| rustc_hir_pretty | 283 | 226 | 79.9 |
| rustc_attr_parsing | 1717 | 1461 | 85.1 |
| rustc_span | 8422 | 7229 | 85.8 |
| rustc_codegen_llvm | 1946 | 1677 | 86.2 |
| rustc_lint | 1337 | 1185 | 88.6 |
| rustc_ast_pretty | 1293 | 1160 | 89.7 |
| rustc_codegen_ssa | 1060 | 966 | 91.1 |
| rustc_session | 3003 | 2739 | 91.2 |
| rustc_ast_ir | 363 | 332 | 91.5 |
| rustc_apfloat | 269 | 247 | 91.8 |
| rustc_const_eval | 1593 | 1464 | 91.9 |
| rustc_hir_id | 338 | 315 | 93.2 |
| rustc_errors | 9626 | 9009 | 93.6 |
| rustc_abi | 2022 | 1893 | 93.6 |
| rustc_hir | 2591 | 2430 | 93.8 |
| rustc_ast | 4141 | 3902 | 94.2 |
| rustc_serialize | 649 | 613 | 94.5 |
| rustc_attr_ir | 505 | 477 | 94.5 |
| rustc_expand | 1293 | 1225 | 94.7 |
| rustc_target | 1667 | 1581 | 94.8 |
| rustc_middle | 33001 | 31455 | 95.3 |
| rustc_type_ir | 7806 | 7478 | 95.8 |
| rustc_borrowck | 1782 | 1714 | 96.2 |
| rustc_trait_selection | 2548 | 2451 | 96.2 |
| rustc_infer | 2286 | 2199 | 96.2 |
| rustc_thread_pool | 239 | 230 | 96.2 |
| rustc_feature | 577 | 558 | 96.7 |
| rustc_error_messages | 450 | 437 | 97.1 |
| rustc_mir_dataflow | 556 | 540 | 97.1 |

## MIR pass effect

73 passes ran; 12 never changed a body in these suites (at any mir-opt-level seen): `CheckCallRecursion`, `CheckConstItemMutation`, `CheckDropRecursion`, `CheckForceInline`, `CheckLiveDrops`, `CheckMutRestriction`, `CheckPackedRef`, `FunctionItemReferences`, `KnownPanicsLint`, `MentionedItems`, `SanityCheck`, `SimplifyConstCondition-final`

| pass | changed bodies of kind | ran on, never changed | mir-opt-levels |
|---|---|---|---|
| `AbortUnwindingCalls` | closure coroutine fn other shim | const promoted static | 0 1 2 3 4 5 |
| `AddCallGuards` | closure coroutine fn other shim | const promoted static | 0 1 2 3 4 5 |
| `AddMovesForPackedDrops` | coroutine fn shim | closure const other promoted static | 0 1 2 3 4 5 |
| `CheckAlignment` | closure coroutine fn shim | other | 0 1 2 3 4 5 |
| `CheckCallRecursion` |  | closure const coroutine fn other static | 0 1 2 3 4 5 |
| `CheckConstItemMutation` |  | closure const coroutine fn other static | 0 1 2 3 4 5 |
| `CheckDropRecursion` |  | closure const coroutine fn other promoted static | 0 1 2 3 4 5 |
| `CheckEnums` | fn | closure coroutine other shim | 0 1 2 3 4 5 |
| `CheckForceInline` |  | closure const coroutine fn other static | 0 1 2 3 4 5 |
| `CheckLiveDrops` |  | const fn promoted | 0 1 2 4 |
| `CheckMutRestriction` |  | closure const coroutine fn other static | 0 1 2 3 4 5 |
| `CheckNull` | closure coroutine fn shim | other | 0 1 2 3 4 5 |
| `CheckPackedRef` |  | closure const coroutine fn other static | 0 1 2 3 4 5 |
| `CleanupPostBorrowck` | closure const coroutine fn other static | promoted | 0 1 2 3 4 5 |
| `CopyProp` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `CtfeLimit` | const fn static | closure | 0 1 2 3 4 |
| `DataflowConstProp` | closure fn | coroutine other shim | 0 1 3 4 5 |
| `DeadStoreElimination-final` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `DeadStoreElimination-initial` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `Derefer` | closure const coroutine fn other static | promoted | 0 1 2 3 4 5 |
| `DestinationPropagation` | closure coroutine fn shim | other | 0 1 2 3 4 5 |
| `EarlyOtherwiseBranch` | fn | closure coroutine other shim | 0 1 2 3 4 5 |
| `ElaborateBoxDerefs` | closure coroutine fn static | const other promoted | 0 1 2 3 4 5 |
| `ElaborateDrops` | closure const coroutine fn other static | promoted | 0 1 2 3 4 5 |
| `EraseDerefTemps` | closure const coroutine fn other static | promoted | 0 1 2 3 4 5 |
| `ForceInline` | shim | closure coroutine fn other | 0 1 2 3 4 5 |
| `FunctionItemReferences` |  | closure const coroutine fn other static | 0 1 2 3 4 5 |
| `GVN` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `ImpossibleClauses` | closure const coroutine fn promoted static | other | 0 1 2 3 4 5 |
| `Inline` | closure fn shim | coroutine other | 0 1 2 3 4 5 |
| `InstSimplify-after-simplifycfg` | closure coroutine fn | other shim | 0 1 2 3 4 5 |
| `InstSimplify-before-inline` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `InstrumentCoverage` | fn |  | 1 |
| `JumpThreading` | closure fn | coroutine other shim | 0 1 2 3 4 5 |
| `KnownPanicsLint` |  | closure const coroutine fn other promoted static | 0 1 2 3 4 5 |
| `LintAndRemoveUninhabited` | closure const coroutine fn static | other | 0 1 2 3 4 5 |
| `LowerIntrinsics` | closure const coroutine fn promoted static | other | 0 1 2 3 4 5 |
| `LowerSliceLenCalls` | closure coroutine fn | other shim | 0 1 2 3 4 5 |
| `MatchBranchSimplification` | closure fn | coroutine other shim | 0 1 2 3 4 5 |
| `MentionedItems` |  | fn shim | 0 1 2 3 4 5 |
| `MultipleReturnTerminators` | closure fn | coroutine other shim | 4 5 |
| `PostAnalysisNormalize` | closure const coroutine fn other promoted static |  | 0 1 2 3 4 5 |
| `PromoteTemps` | closure const coroutine fn other static |  | 0 1 2 3 4 5 |
| `ReferencePropagation` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `RemoveNoopLandingPads` | closure const coroutine fn other shim static | promoted | 0 1 2 3 4 5 |
| `RemovePlaceMention` | closure const coroutine fn other static | promoted | 0 1 2 3 4 5 |
| `RemoveStorageMarkers` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `RemoveUninitDrops` | const fn | promoted | 0 1 2 4 |
| `RemoveUnneededDrops` | closure fn shim | coroutine other | 0 1 2 3 4 5 |
| `RemoveZsts` | closure fn | coroutine other shim | 0 1 2 3 4 5 |
| `SanityCheck` |  | closure const coroutine fn other static | 0 1 2 3 4 5 |
| `ScalarReplacementOfAggregates` | closure fn | coroutine other shim | 0 1 2 3 4 5 |
| `SimplifyCfg-after-unreachable-enum-branching` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `SimplifyCfg-final` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `SimplifyCfg-initial` | closure const coroutine fn other static |  | 0 1 2 3 4 5 |
| `SimplifyCfg-make_shim` | shim | fn | 0 1 2 3 4 5 |
| `SimplifyCfg-post-analysis` | closure const coroutine fn other static | promoted | 0 1 2 3 4 5 |
| `SimplifyCfg-pre-optimizations` | closure const coroutine fn other promoted static |  | 0 1 2 3 4 5 |
| `SimplifyCfg-promote-consts` | const coroutine fn static | closure other | 0 1 2 3 4 5 |
| `SimplifyCfg-remove-false-edges` | const fn | promoted | 0 1 2 4 |
| `SimplifyComparisonIntegral` | closure coroutine fn | other shim | 0 1 2 3 4 5 |
| `SimplifyConstCondition-after-const-prop` | closure coroutine fn | other shim | 0 1 2 3 4 5 |
| `SimplifyConstCondition-after-inst-simplify` | closure coroutine fn | other shim | 0 1 2 3 4 5 |
| `SimplifyConstCondition-final` |  | closure coroutine fn other shim | 0 1 2 3 4 5 |
| `SimplifyLocals-after-value-numbering` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `SimplifyLocals-before-const-prop` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `SimplifyLocals-final` | closure coroutine fn other shim |  | 0 1 2 3 4 5 |
| `SingleUseConsts` | closure coroutine fn other | shim | 0 1 2 3 4 5 |
| `SsaRangePropagation` | fn | closure coroutine other shim | 0 1 2 3 4 5 |
| `StateTransform` | coroutine other shim | closure const fn promoted static | 0 1 2 3 4 5 |
| `Subtyper` | closure const fn | coroutine other promoted static | 0 1 2 3 4 5 |
| `UnreachableEnumBranching` | closure coroutine fn shim | other | 0 1 2 3 4 5 |
| `UnreachablePropagation` | closure coroutine fn | other shim | 0 1 2 3 4 5 |

## Lock contention

37 `Lock::lock` call sites were found held at least once under `-Zthreads`:

- `compiler/rustc_data_structures/src/sharded.rs:164`
- `compiler/rustc_data_structures/src/sharded.rs:195`
- `compiler/rustc_data_structures/src/sharded.rs:221`
- `compiler/rustc_data_structures/src/sharded.rs:240`
- `compiler/rustc_data_structures/src/sharded.rs:261`
- `compiler/rustc_errors/src/lib.rs:510`
- `compiler/rustc_errors/src/lib.rs:732`
- `compiler/rustc_errors/src/lib.rs:867`
- `compiler/rustc_infer/src/infer/canonical/instantiate.rs:149`
- `compiler/rustc_infer/src/infer/canonical/instantiate.rs:154`
- `compiler/rustc_infer/src/infer/canonical/instantiate.rs:158`
- `compiler/rustc_interface/src/callbacks.rs:64`
- `compiler/rustc_interface/src/callbacks.rs:70`
- `compiler/rustc_metadata/src/rmeta/decoder.rs:1627`
- `compiler/rustc_metadata/src/rmeta/decoder.rs:1812`
- `compiler/rustc_metadata/src/rmeta/decoder.rs:400`
- `compiler/rustc_metadata/src/rmeta/decoder.rs:405`
- `compiler/rustc_middle/src/infer/canonical.rs:187`
- `compiler/rustc_middle/src/mir/interpret/error.rs:176`
- `compiler/rustc_middle/src/mir/interpret/mod.rs:555`
- `compiler/rustc_middle/src/traits/cache.rs:29`
- `compiler/rustc_middle/src/traits/cache.rs:33`
- `compiler/rustc_middle/src/ty/context/impl_interner.rs:145`
- `compiler/rustc_middle/src/ty/context/impl_interner.rs:152`
- `compiler/rustc_monomorphize/src/collector.rs:361`
- `compiler/rustc_monomorphize/src/collector.rs:576`
- `compiler/rustc_monomorphize/src/collector.rs:584`
- `compiler/rustc_query_impl/src/execution.rs:123`
- `compiler/rustc_query_impl/src/execution.rs:287`
- `compiler/rustc_session/src/code_stats.rs:108`
- `compiler/rustc_session/src/session.rs:687`
- `compiler/rustc_span/src/hygiene.rs:1516`
- `compiler/rustc_span/src/hygiene.rs:1526`
- `compiler/rustc_span/src/hygiene.rs:394`
- `compiler/rustc_span/src/span_encoding.rs:529`
- `compiler/rustc_span/src/symbol.rs:2936`
- `compiler/rustc_span/src/symbol.rs:2975`

