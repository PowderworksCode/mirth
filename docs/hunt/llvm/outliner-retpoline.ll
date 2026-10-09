; ModuleID = 'a.334303bdd0b22dee-cgu.0'
source_filename = "a.334303bdd0b22dee-cgu.0"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

; core::ptr::drop_glue::<alloc::boxed::Box<dyn core::ops::function::Fn<(u32,), Output = u32>>>
; Function Attrs: nonlazybind uwtable
define internal fastcc void @_RINvNtCslK5Drzrx8K4_4core3ptr9drop_glueINtNtCsdf08ABbzq28_5alloc5boxed3BoxDINtNtNtB4_3ops8function2FnTmEEp6OutputmEL_EECs4oRDWwiVKk4_1a(ptr noalias nofree noundef nonnull readonly align 8 captures(none) dereferenceable(16) %_1) unnamed_addr #0 personality ptr @rust_eh_personality !guid !4 {
start:
  %0 = getelementptr inbounds nuw i8, ptr %_1, i64 8
  %_6.1 = load ptr, ptr %0, align 8, !nonnull !5, !align !6, !noundef !5
  %1 = load ptr, ptr %_6.1, align 8, !invariant.load !5
  %.not = icmp eq ptr %1, null
  br i1 %.not, label %bb3, label %is_not_null

is_not_null:                                      ; preds = %start
  %_6.0 = load ptr, ptr %_1, align 8, !nonnull !5, !noundef !5
  invoke void %1(ptr noundef nonnull %_6.0)
          to label %bb3 unwind label %cleanup

bb3:                                              ; preds = %is_not_null, %start
  tail call void @llvm.experimental.noalias.scope.decl(metadata !7)
  %2 = getelementptr inbounds nuw i8, ptr %_6.1, i64 8
  %size.i = load i64, ptr %2, align 8, !range !10, !invariant.load !5, !noalias !7
  %3 = icmp eq i64 %size.i, 0
  br i1 %3, label %_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a.exit, label %bb1.i

bb1.i:                                            ; preds = %bb3
  %4 = getelementptr inbounds nuw i8, ptr %_6.1, i64 16
  %align.i = load i64, ptr %4, align 8, !range !11, !invariant.load !5, !noalias !7
  %ptr.0.i = load ptr, ptr %_1, align 8, !alias.scope !7, !nonnull !5, !noundef !5
; call __rustc::__rust_dealloc
  tail call void @_RNvCsjMmYDGKcCRn_7___rustc14___rust_dealloc(ptr noundef nonnull %ptr.0.i, i64 noundef range(i64 1, -9223372036854775808) %size.i, i64 noundef range(i64 1, 536870913) %align.i) #8, !noalias !7
  br label %_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a.exit

_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a.exit: ; preds = %bb3, %bb1.i
  ret void

cleanup:                                          ; preds = %is_not_null
  %5 = landingpad { ptr, i32 }
          cleanup
  %6 = getelementptr inbounds nuw i8, ptr %_6.1, i64 8
  %size.i2 = load i64, ptr %6, align 8, !range !10, !invariant.load !5, !noalias !12
  %7 = icmp eq i64 %size.i2, 0
  br i1 %7, label %bb2, label %bb1.i3

bb1.i3:                                           ; preds = %cleanup
  %8 = getelementptr inbounds nuw i8, ptr %_6.1, i64 16
  %align.i4 = load i64, ptr %8, align 8, !range !11, !invariant.load !5, !noalias !12
; call __rustc::__rust_dealloc
  tail call void @_RNvCsjMmYDGKcCRn_7___rustc14___rust_dealloc(ptr noundef nonnull %_6.0, i64 noundef range(i64 1, -9223372036854775808) %size.i2, i64 noundef range(i64 1, 536870913) %align.i4) #8, !noalias !12
  br label %bb2

bb2:                                              ; preds = %bb1.i3, %cleanup
  resume { ptr, i32 } %5
}

; <core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::f::{closure#0}> as core::iter::traits::iterator::Iterator>::fold::<u32, <u32 as core::iter::traits::accum::Sum>::sum<core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::f::{closure#0}>>::{closure#0}>
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read, inaccessiblemem: write) uwtable
define noundef i32 @_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_(ptr noundef nonnull %self.0, ptr noundef %self.1, i32 noundef %init) unnamed_addr #1 personality ptr @rust_eh_personality !guid !15 {
start:
  call void @llvm.assume(i1 true) [ "nonnull"(ptr %self.1) ]
  %0 = icmp eq ptr %self.0, %self.1
  br i1 %0, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit, label %bb5.i

bb5.i:                                            ; preds = %start
  %1 = ptrtoint ptr %self.1 to i64
  %2 = ptrtoint ptr %self.0 to i64
  %3 = sub nuw i64 %1, %2
  %4 = lshr exact i64 %3, 2
  %xtraiter = and i64 %4, 3
  %5 = icmp ult i64 %3, 16
  br i1 %5, label %bb8.i.epil.preheader, label %bb5.i.new

bb5.i.new:                                        ; preds = %bb5.i
  %unroll_iter = and i64 %4, 4611686018427387900
  br label %bb8.i

bb8.i:                                            ; preds = %bb8.i, %bb5.i.new
  %i.sroa.0.0.i = phi i64 [ 0, %bb5.i.new ], [ %_27.i.3, %bb8.i ]
  %acc.sroa.0.0.i = phi i32 [ %init, %bb5.i.new ], [ %_4.0.i.i.i.3, %bb8.i ]
  %niter = phi i64 [ 0, %bb5.i.new ], [ %niter.next.3, %bb8.i ]
  %_45.i = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %self1.i.i.i.i = load i32, ptr %_45.i, align 4, !alias.scope !16, !noundef !5
  %_4.0.i.i.i.i = mul i32 %self1.i.i.i.i, 3
  %_4.0.i.i.i = add i32 %_4.0.i.i.i.i, %acc.sroa.0.0.i
  %6 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %_45.i.1 = getelementptr inbounds nuw i8, ptr %6, i64 4
  %self1.i.i.i.i.1 = load i32, ptr %_45.i.1, align 4, !alias.scope !16, !noundef !5
  %_4.0.i.i.i.i.1 = mul i32 %self1.i.i.i.i.1, 3
  %_4.0.i.i.i.1 = add i32 %_4.0.i.i.i.i.1, %_4.0.i.i.i
  %7 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %_45.i.2 = getelementptr inbounds nuw i8, ptr %7, i64 8
  %self1.i.i.i.i.2 = load i32, ptr %_45.i.2, align 4, !alias.scope !16, !noundef !5
  %_4.0.i.i.i.i.2 = mul i32 %self1.i.i.i.i.2, 3
  %_4.0.i.i.i.2 = add i32 %_4.0.i.i.i.i.2, %_4.0.i.i.i.1
  %8 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %_45.i.3 = getelementptr inbounds nuw i8, ptr %8, i64 12
  %self1.i.i.i.i.3 = load i32, ptr %_45.i.3, align 4, !alias.scope !16, !noundef !5
  %_4.0.i.i.i.i.3 = mul i32 %self1.i.i.i.i.3, 3
  %_4.0.i.i.i.3 = add i32 %_4.0.i.i.i.i.3, %_4.0.i.i.i.2
  %_27.i.3 = add nuw i64 %i.sroa.0.0.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa, label %bb8.i

_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa: ; preds = %bb8.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit, label %bb8.i.epil.preheader

bb8.i.epil.preheader:                             ; preds = %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa, %bb5.i
  %i.sroa.0.0.i.epil.init = phi i64 [ 0, %bb5.i ], [ %_27.i.3, %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.epil.init = phi i32 [ %init, %bb5.i ], [ %_4.0.i.i.i.3, %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa ]
  %lcmp.mod2 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod2)
  br label %bb8.i.epil

bb8.i.epil:                                       ; preds = %bb8.i.epil, %bb8.i.epil.preheader
  %i.sroa.0.0.i.epil = phi i64 [ %i.sroa.0.0.i.epil.init, %bb8.i.epil.preheader ], [ %_27.i.epil, %bb8.i.epil ]
  %acc.sroa.0.0.i.epil = phi i32 [ %acc.sroa.0.0.i.epil.init, %bb8.i.epil.preheader ], [ %_4.0.i.i.i.epil, %bb8.i.epil ]
  %epil.iter = phi i64 [ 0, %bb8.i.epil.preheader ], [ %epil.iter.next, %bb8.i.epil ]
  %_45.i.epil = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.epil
  %self1.i.i.i.i.epil = load i32, ptr %_45.i.epil, align 4, !alias.scope !16, !noundef !5
  %_4.0.i.i.i.i.epil = mul i32 %self1.i.i.i.i.epil, 3
  %_4.0.i.i.i.epil = add i32 %_4.0.i.i.i.i.epil, %acc.sroa.0.0.i.epil
  %_27.i.epil = add nuw i64 %i.sroa.0.0.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit, label %bb8.i.epil, !llvm.loop !23

_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit: ; preds = %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa, %bb8.i.epil, %start
  %_0.sroa.0.0.i = phi i32 [ %init, %start ], [ %_4.0.i.i.i.3, %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.epil, %bb8.i.epil ]
  ret i32 %_0.sroa.0.0.i
}

; <core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::g::{closure#0}> as core::iter::traits::iterator::Iterator>::fold::<u32, <u32 as core::iter::traits::accum::Sum>::sum<core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::g::{closure#0}>>::{closure#0}>
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read, inaccessiblemem: write) uwtable
define noundef i32 @_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_(ptr noundef nonnull %self.0, ptr noundef %self.1, i32 noundef %init) unnamed_addr #1 personality ptr @rust_eh_personality !guid !25 {
start:
  call void @llvm.assume(i1 true) [ "nonnull"(ptr %self.1) ]
  %0 = icmp eq ptr %self.0, %self.1
  br i1 %0, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit, label %bb5.i

bb5.i:                                            ; preds = %start
  %1 = ptrtoint ptr %self.1 to i64
  %2 = ptrtoint ptr %self.0 to i64
  %3 = sub nuw i64 %1, %2
  %4 = lshr exact i64 %3, 2
  %xtraiter = and i64 %4, 3
  %5 = icmp ult i64 %3, 16
  br i1 %5, label %bb8.i.epil.preheader, label %bb5.i.new

bb5.i.new:                                        ; preds = %bb5.i
  %unroll_iter = and i64 %4, 4611686018427387900
  br label %bb8.i

bb8.i:                                            ; preds = %bb8.i, %bb5.i.new
  %i.sroa.0.0.i = phi i64 [ 0, %bb5.i.new ], [ %_27.i.3, %bb8.i ]
  %acc.sroa.0.0.i = phi i32 [ %init, %bb5.i.new ], [ %_4.0.i.i.i.3, %bb8.i ]
  %niter = phi i64 [ 0, %bb5.i.new ], [ %niter.next.3, %bb8.i ]
  %_45.i = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %self1.i.i.i.i = load i32, ptr %_45.i, align 4, !alias.scope !26, !noundef !5
  %_4.0.i.i.i.i = mul i32 %self1.i.i.i.i, 5
  %_4.0.i.i.i = add i32 %_4.0.i.i.i.i, %acc.sroa.0.0.i
  %6 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %_45.i.1 = getelementptr inbounds nuw i8, ptr %6, i64 4
  %self1.i.i.i.i.1 = load i32, ptr %_45.i.1, align 4, !alias.scope !26, !noundef !5
  %_4.0.i.i.i.i.1 = mul i32 %self1.i.i.i.i.1, 5
  %_4.0.i.i.i.1 = add i32 %_4.0.i.i.i.i.1, %_4.0.i.i.i
  %7 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %_45.i.2 = getelementptr inbounds nuw i8, ptr %7, i64 8
  %self1.i.i.i.i.2 = load i32, ptr %_45.i.2, align 4, !alias.scope !26, !noundef !5
  %_4.0.i.i.i.i.2 = mul i32 %self1.i.i.i.i.2, 5
  %_4.0.i.i.i.2 = add i32 %_4.0.i.i.i.i.2, %_4.0.i.i.i.1
  %8 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i
  %_45.i.3 = getelementptr inbounds nuw i8, ptr %8, i64 12
  %self1.i.i.i.i.3 = load i32, ptr %_45.i.3, align 4, !alias.scope !26, !noundef !5
  %_4.0.i.i.i.i.3 = mul i32 %self1.i.i.i.i.3, 5
  %_4.0.i.i.i.3 = add i32 %_4.0.i.i.i.i.3, %_4.0.i.i.i.2
  %_27.i.3 = add nuw i64 %i.sroa.0.0.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa, label %bb8.i

_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa: ; preds = %bb8.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit, label %bb8.i.epil.preheader

bb8.i.epil.preheader:                             ; preds = %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa, %bb5.i
  %i.sroa.0.0.i.epil.init = phi i64 [ 0, %bb5.i ], [ %_27.i.3, %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.epil.init = phi i32 [ %init, %bb5.i ], [ %_4.0.i.i.i.3, %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa ]
  %lcmp.mod2 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod2)
  br label %bb8.i.epil

bb8.i.epil:                                       ; preds = %bb8.i.epil, %bb8.i.epil.preheader
  %i.sroa.0.0.i.epil = phi i64 [ %i.sroa.0.0.i.epil.init, %bb8.i.epil.preheader ], [ %_27.i.epil, %bb8.i.epil ]
  %acc.sroa.0.0.i.epil = phi i32 [ %acc.sroa.0.0.i.epil.init, %bb8.i.epil.preheader ], [ %_4.0.i.i.i.epil, %bb8.i.epil ]
  %epil.iter = phi i64 [ 0, %bb8.i.epil.preheader ], [ %epil.iter.next, %bb8.i.epil ]
  %_45.i.epil = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.epil
  %self1.i.i.i.i.epil = load i32, ptr %_45.i.epil, align 4, !alias.scope !26, !noundef !5
  %_4.0.i.i.i.i.epil = mul i32 %self1.i.i.i.i.epil, 5
  %_4.0.i.i.i.epil = add i32 %_4.0.i.i.i.i.epil, %acc.sroa.0.0.i.epil
  %_27.i.epil = add nuw i64 %i.sroa.0.0.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit, label %bb8.i.epil, !llvm.loop !33

_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit: ; preds = %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa, %bb8.i.epil, %start
  %_0.sroa.0.0.i = phi i32 [ %init, %start ], [ %_4.0.i.i.i.3, %_RINvXs2J_NtNtCslK5Drzrx8K4_4core5slice4iterINtB7_4ItermENtNtNtNtBb_4iter6traits8iterator8Iterator4foldmNCINvNtNtBY_8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtBW_5accummNtB2L_3Sum3sumINtB1I_3MapBF_B2f_EE0E0EB2j_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.epil, %bb8.i.epil ]
  ret i32 %_0.sroa.0.0.i
}

; <u32 as core::iter::traits::accum::Sum>::sum::<core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::f::{closure#0}>>
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read, inaccessiblemem: write) uwtable
define noundef i32 @_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_(ptr noundef nonnull %iter.0, ptr noundef %iter.1) unnamed_addr #1 personality ptr @rust_eh_personality !guid !34 {
start:
  call void @llvm.assume(i1 true) [ "nonnull"(ptr %iter.1) ]
  %0 = icmp eq ptr %iter.0, %iter.1
  br i1 %0, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit, label %bb5.i.i

bb5.i.i:                                          ; preds = %start
  %1 = ptrtoint ptr %iter.1 to i64
  %2 = ptrtoint ptr %iter.0 to i64
  %3 = sub nuw i64 %1, %2
  %4 = lshr exact i64 %3, 2
  %xtraiter = and i64 %4, 3
  %5 = icmp ult i64 %3, 16
  br i1 %5, label %bb8.i.i.epil.preheader, label %bb5.i.i.new

bb5.i.i.new:                                      ; preds = %bb5.i.i
  %unroll_iter = and i64 %4, 4611686018427387900
  br label %bb8.i.i

bb8.i.i:                                          ; preds = %bb8.i.i, %bb5.i.i.new
  %i.sroa.0.0.i.i = phi i64 [ 0, %bb5.i.i.new ], [ %_27.i.i.3, %bb8.i.i ]
  %acc.sroa.0.0.i.i = phi i32 [ 0, %bb5.i.i.new ], [ %_4.0.i.i.i.i.3, %bb8.i.i ]
  %niter = phi i64 [ 0, %bb5.i.i.new ], [ %niter.next.3, %bb8.i.i ]
  %_45.i.i = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %self1.i.i.i.i.i = load i32, ptr %_45.i.i, align 4, !alias.scope !35, !noundef !5
  %_4.0.i.i.i.i.i = mul i32 %self1.i.i.i.i.i, 3
  %_4.0.i.i.i.i = add i32 %_4.0.i.i.i.i.i, %acc.sroa.0.0.i.i
  %6 = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %_45.i.i.1 = getelementptr inbounds nuw i8, ptr %6, i64 4
  %self1.i.i.i.i.i.1 = load i32, ptr %_45.i.i.1, align 4, !alias.scope !35, !noundef !5
  %_4.0.i.i.i.i.i.1 = mul i32 %self1.i.i.i.i.i.1, 3
  %_4.0.i.i.i.i.1 = add i32 %_4.0.i.i.i.i.i.1, %_4.0.i.i.i.i
  %7 = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %_45.i.i.2 = getelementptr inbounds nuw i8, ptr %7, i64 8
  %self1.i.i.i.i.i.2 = load i32, ptr %_45.i.i.2, align 4, !alias.scope !35, !noundef !5
  %_4.0.i.i.i.i.i.2 = mul i32 %self1.i.i.i.i.i.2, 3
  %_4.0.i.i.i.i.2 = add i32 %_4.0.i.i.i.i.i.2, %_4.0.i.i.i.i.1
  %8 = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %_45.i.i.3 = getelementptr inbounds nuw i8, ptr %8, i64 12
  %self1.i.i.i.i.i.3 = load i32, ptr %_45.i.i.3, align 4, !alias.scope !35, !noundef !5
  %_4.0.i.i.i.i.i.3 = mul i32 %self1.i.i.i.i.i.3, 3
  %_4.0.i.i.i.i.3 = add i32 %_4.0.i.i.i.i.i.3, %_4.0.i.i.i.i.2
  %_27.i.i.3 = add nuw i64 %i.sroa.0.0.i.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa, label %bb8.i.i

_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa: ; preds = %bb8.i.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit, label %bb8.i.i.epil.preheader

bb8.i.i.epil.preheader:                           ; preds = %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa, %bb5.i.i
  %i.sroa.0.0.i.i.epil.init = phi i64 [ 0, %bb5.i.i ], [ %_27.i.i.3, %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.i.epil.init = phi i32 [ 0, %bb5.i.i ], [ %_4.0.i.i.i.i.3, %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa ]
  %lcmp.mod2 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod2)
  br label %bb8.i.i.epil

bb8.i.i.epil:                                     ; preds = %bb8.i.i.epil, %bb8.i.i.epil.preheader
  %i.sroa.0.0.i.i.epil = phi i64 [ %i.sroa.0.0.i.i.epil.init, %bb8.i.i.epil.preheader ], [ %_27.i.i.epil, %bb8.i.i.epil ]
  %acc.sroa.0.0.i.i.epil = phi i32 [ %acc.sroa.0.0.i.i.epil.init, %bb8.i.i.epil.preheader ], [ %_4.0.i.i.i.i.epil, %bb8.i.i.epil ]
  %epil.iter = phi i64 [ 0, %bb8.i.i.epil.preheader ], [ %epil.iter.next, %bb8.i.i.epil ]
  %_45.i.i.epil = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i.epil
  %self1.i.i.i.i.i.epil = load i32, ptr %_45.i.i.epil, align 4, !alias.scope !35, !noundef !5
  %_4.0.i.i.i.i.i.epil = mul i32 %self1.i.i.i.i.i.epil, 3
  %_4.0.i.i.i.i.epil = add i32 %_4.0.i.i.i.i.i.epil, %acc.sroa.0.0.i.i.epil
  %_27.i.i.epil = add nuw i64 %i.sroa.0.0.i.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit, label %bb8.i.i.epil, !llvm.loop !42

_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit: ; preds = %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa, %bb8.i.i.epil, %start
  %_0.sroa.0.0.i.i = phi i32 [ 0, %start ], [ %_4.0.i.i.i.i.3, %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.i.epil, %bb8.i.i.epil ]
  ret i32 %_0.sroa.0.0.i.i
}

; <u32 as core::iter::traits::accum::Sum>::sum::<core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::g::{closure#0}>>
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read, inaccessiblemem: write) uwtable
define noundef i32 @_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_(ptr noundef nonnull %iter.0, ptr noundef %iter.1) unnamed_addr #1 personality ptr @rust_eh_personality !guid !43 {
start:
  call void @llvm.assume(i1 true) [ "nonnull"(ptr %iter.1) ]
  %0 = icmp eq ptr %iter.0, %iter.1
  br i1 %0, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit, label %bb5.i.i

bb5.i.i:                                          ; preds = %start
  %1 = ptrtoint ptr %iter.1 to i64
  %2 = ptrtoint ptr %iter.0 to i64
  %3 = sub nuw i64 %1, %2
  %4 = lshr exact i64 %3, 2
  %xtraiter = and i64 %4, 3
  %5 = icmp ult i64 %3, 16
  br i1 %5, label %bb8.i.i.epil.preheader, label %bb5.i.i.new

bb5.i.i.new:                                      ; preds = %bb5.i.i
  %unroll_iter = and i64 %4, 4611686018427387900
  br label %bb8.i.i

bb8.i.i:                                          ; preds = %bb8.i.i, %bb5.i.i.new
  %i.sroa.0.0.i.i = phi i64 [ 0, %bb5.i.i.new ], [ %_27.i.i.3, %bb8.i.i ]
  %acc.sroa.0.0.i.i = phi i32 [ 0, %bb5.i.i.new ], [ %_4.0.i.i.i.i.3, %bb8.i.i ]
  %niter = phi i64 [ 0, %bb5.i.i.new ], [ %niter.next.3, %bb8.i.i ]
  %_45.i.i = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %self1.i.i.i.i.i = load i32, ptr %_45.i.i, align 4, !alias.scope !44, !noundef !5
  %_4.0.i.i.i.i.i = mul i32 %self1.i.i.i.i.i, 5
  %_4.0.i.i.i.i = add i32 %_4.0.i.i.i.i.i, %acc.sroa.0.0.i.i
  %6 = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %_45.i.i.1 = getelementptr inbounds nuw i8, ptr %6, i64 4
  %self1.i.i.i.i.i.1 = load i32, ptr %_45.i.i.1, align 4, !alias.scope !44, !noundef !5
  %_4.0.i.i.i.i.i.1 = mul i32 %self1.i.i.i.i.i.1, 5
  %_4.0.i.i.i.i.1 = add i32 %_4.0.i.i.i.i.i.1, %_4.0.i.i.i.i
  %7 = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %_45.i.i.2 = getelementptr inbounds nuw i8, ptr %7, i64 8
  %self1.i.i.i.i.i.2 = load i32, ptr %_45.i.i.2, align 4, !alias.scope !44, !noundef !5
  %_4.0.i.i.i.i.i.2 = mul i32 %self1.i.i.i.i.i.2, 5
  %_4.0.i.i.i.i.2 = add i32 %_4.0.i.i.i.i.i.2, %_4.0.i.i.i.i.1
  %8 = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i
  %_45.i.i.3 = getelementptr inbounds nuw i8, ptr %8, i64 12
  %self1.i.i.i.i.i.3 = load i32, ptr %_45.i.i.3, align 4, !alias.scope !44, !noundef !5
  %_4.0.i.i.i.i.i.3 = mul i32 %self1.i.i.i.i.i.3, 5
  %_4.0.i.i.i.i.3 = add i32 %_4.0.i.i.i.i.i.3, %_4.0.i.i.i.i.2
  %_27.i.i.3 = add nuw i64 %i.sroa.0.0.i.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa, label %bb8.i.i

_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa: ; preds = %bb8.i.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit, label %bb8.i.i.epil.preheader

bb8.i.i.epil.preheader:                           ; preds = %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa, %bb5.i.i
  %i.sroa.0.0.i.i.epil.init = phi i64 [ 0, %bb5.i.i ], [ %_27.i.i.3, %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.i.epil.init = phi i32 [ 0, %bb5.i.i ], [ %_4.0.i.i.i.i.3, %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa ]
  %lcmp.mod2 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod2)
  br label %bb8.i.i.epil

bb8.i.i.epil:                                     ; preds = %bb8.i.i.epil, %bb8.i.i.epil.preheader
  %i.sroa.0.0.i.i.epil = phi i64 [ %i.sroa.0.0.i.i.epil.init, %bb8.i.i.epil.preheader ], [ %_27.i.i.epil, %bb8.i.i.epil ]
  %acc.sroa.0.0.i.i.epil = phi i32 [ %acc.sroa.0.0.i.i.epil.init, %bb8.i.i.epil.preheader ], [ %_4.0.i.i.i.i.epil, %bb8.i.i.epil ]
  %epil.iter = phi i64 [ 0, %bb8.i.i.epil.preheader ], [ %epil.iter.next, %bb8.i.i.epil ]
  %_45.i.i.epil = getelementptr inbounds nuw [4 x i8], ptr %iter.0, i64 %i.sroa.0.0.i.i.epil
  %self1.i.i.i.i.i.epil = load i32, ptr %_45.i.i.epil, align 4, !alias.scope !44, !noundef !5
  %_4.0.i.i.i.i.i.epil = mul i32 %self1.i.i.i.i.i.epil, 5
  %_4.0.i.i.i.i.epil = add i32 %_4.0.i.i.i.i.i.epil, %acc.sroa.0.0.i.i.epil
  %_27.i.i.epil = add nuw i64 %i.sroa.0.0.i.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit, label %bb8.i.i.epil, !llvm.loop !51

_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit: ; preds = %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa, %bb8.i.i.epil, %start
  %_0.sroa.0.0.i.i = phi i32 [ 0, %start ], [ %_4.0.i.i.i.i.3, %_RINvXs0_NtNtNtCslK5Drzrx8K4_4core4iter8adapters3mapINtB6_3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator4foldmNCINvXsy_NtB1R_5accummNtB2A_3Sum3sumBN_E0EB1t_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.i.epil, %bb8.i.i.epil ]
  ret i32 %_0.sroa.0.0.i.i
}

; <core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::f::{closure#0}> as core::iter::traits::iterator::Iterator>::sum::<u32>
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read, inaccessiblemem: write) uwtable
define noundef i32 @_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_(ptr noundef nonnull %self.0, ptr noundef %self.1) unnamed_addr #1 personality ptr @rust_eh_personality !guid !52 {
start:
  call void @llvm.assume(i1 true) [ "nonnull"(ptr %self.1) ]
  %0 = icmp eq ptr %self.0, %self.1
  br i1 %0, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit, label %bb5.i.i.i

bb5.i.i.i:                                        ; preds = %start
  %1 = ptrtoint ptr %self.1 to i64
  %2 = ptrtoint ptr %self.0 to i64
  %3 = sub nuw i64 %1, %2
  %4 = lshr exact i64 %3, 2
  %xtraiter = and i64 %4, 3
  %5 = icmp ult i64 %3, 16
  br i1 %5, label %bb8.i.i.i.epil.preheader, label %bb5.i.i.i.new

bb5.i.i.i.new:                                    ; preds = %bb5.i.i.i
  %unroll_iter = and i64 %4, 4611686018427387900
  br label %bb8.i.i.i

bb8.i.i.i:                                        ; preds = %bb8.i.i.i, %bb5.i.i.i.new
  %i.sroa.0.0.i.i.i = phi i64 [ 0, %bb5.i.i.i.new ], [ %_27.i.i.i.3, %bb8.i.i.i ]
  %acc.sroa.0.0.i.i.i = phi i32 [ 0, %bb5.i.i.i.new ], [ %_4.0.i.i.i.i.i.3, %bb8.i.i.i ]
  %niter = phi i64 [ 0, %bb5.i.i.i.new ], [ %niter.next.3, %bb8.i.i.i ]
  %_45.i.i.i = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %self1.i.i.i.i.i.i = load i32, ptr %_45.i.i.i, align 4, !alias.scope !53, !noundef !5
  %_4.0.i.i.i.i.i.i = mul i32 %self1.i.i.i.i.i.i, 3
  %_4.0.i.i.i.i.i = add i32 %_4.0.i.i.i.i.i.i, %acc.sroa.0.0.i.i.i
  %6 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %_45.i.i.i.1 = getelementptr inbounds nuw i8, ptr %6, i64 4
  %self1.i.i.i.i.i.i.1 = load i32, ptr %_45.i.i.i.1, align 4, !alias.scope !53, !noundef !5
  %_4.0.i.i.i.i.i.i.1 = mul i32 %self1.i.i.i.i.i.i.1, 3
  %_4.0.i.i.i.i.i.1 = add i32 %_4.0.i.i.i.i.i.i.1, %_4.0.i.i.i.i.i
  %7 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %_45.i.i.i.2 = getelementptr inbounds nuw i8, ptr %7, i64 8
  %self1.i.i.i.i.i.i.2 = load i32, ptr %_45.i.i.i.2, align 4, !alias.scope !53, !noundef !5
  %_4.0.i.i.i.i.i.i.2 = mul i32 %self1.i.i.i.i.i.i.2, 3
  %_4.0.i.i.i.i.i.2 = add i32 %_4.0.i.i.i.i.i.i.2, %_4.0.i.i.i.i.i.1
  %8 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %_45.i.i.i.3 = getelementptr inbounds nuw i8, ptr %8, i64 12
  %self1.i.i.i.i.i.i.3 = load i32, ptr %_45.i.i.i.3, align 4, !alias.scope !53, !noundef !5
  %_4.0.i.i.i.i.i.i.3 = mul i32 %self1.i.i.i.i.i.i.3, 3
  %_4.0.i.i.i.i.i.3 = add i32 %_4.0.i.i.i.i.i.i.3, %_4.0.i.i.i.i.i.2
  %_27.i.i.i.3 = add nuw i64 %i.sroa.0.0.i.i.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit.loopexit.unr-lcssa, label %bb8.i.i.i

_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit.loopexit.unr-lcssa: ; preds = %bb8.i.i.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit, label %bb8.i.i.i.epil.preheader

bb8.i.i.i.epil.preheader:                         ; preds = %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit.loopexit.unr-lcssa, %bb5.i.i.i
  %i.sroa.0.0.i.i.i.epil.init = phi i64 [ 0, %bb5.i.i.i ], [ %_27.i.i.i.3, %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.i.i.epil.init = phi i32 [ 0, %bb5.i.i.i ], [ %_4.0.i.i.i.i.i.3, %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit.loopexit.unr-lcssa ]
  %lcmp.mod2 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod2)
  br label %bb8.i.i.i.epil

bb8.i.i.i.epil:                                   ; preds = %bb8.i.i.i.epil, %bb8.i.i.i.epil.preheader
  %i.sroa.0.0.i.i.i.epil = phi i64 [ %i.sroa.0.0.i.i.i.epil.init, %bb8.i.i.i.epil.preheader ], [ %_27.i.i.i.epil, %bb8.i.i.i.epil ]
  %acc.sroa.0.0.i.i.i.epil = phi i32 [ %acc.sroa.0.0.i.i.i.epil.init, %bb8.i.i.i.epil.preheader ], [ %_4.0.i.i.i.i.i.epil, %bb8.i.i.i.epil ]
  %epil.iter = phi i64 [ 0, %bb8.i.i.i.epil.preheader ], [ %epil.iter.next, %bb8.i.i.i.epil ]
  %_45.i.i.i.epil = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i.epil
  %self1.i.i.i.i.i.i.epil = load i32, ptr %_45.i.i.i.epil, align 4, !alias.scope !53, !noundef !5
  %_4.0.i.i.i.i.i.i.epil = mul i32 %self1.i.i.i.i.i.i.epil, 3
  %_4.0.i.i.i.i.i.epil = add i32 %_4.0.i.i.i.i.i.i.epil, %acc.sroa.0.0.i.i.i.epil
  %_27.i.i.i.epil = add nuw i64 %i.sroa.0.0.i.i.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit, label %bb8.i.i.i.epil, !llvm.loop !60

_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit: ; preds = %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit.loopexit.unr-lcssa, %bb8.i.i.i.epil, %start
  %_0.sroa.0.0.i.i.i = phi i32 [ 0, %start ], [ %_4.0.i.i.i.i.i.3, %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0EEB1Y_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.i.i.epil, %bb8.i.i.i.epil ]
  ret i32 %_0.sroa.0.0.i.i.i
}

; <core::iter::adapters::map::Map<core::slice::iter::Iter<u32>, a::g::{closure#0}> as core::iter::traits::iterator::Iterator>::sum::<u32>
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read, inaccessiblemem: write) uwtable
define noundef i32 @_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_(ptr noundef nonnull %self.0, ptr noundef %self.1) unnamed_addr #1 personality ptr @rust_eh_personality !guid !61 {
start:
  call void @llvm.assume(i1 true) [ "nonnull"(ptr %self.1) ]
  %0 = icmp eq ptr %self.0, %self.1
  br i1 %0, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit, label %bb5.i.i.i

bb5.i.i.i:                                        ; preds = %start
  %1 = ptrtoint ptr %self.1 to i64
  %2 = ptrtoint ptr %self.0 to i64
  %3 = sub nuw i64 %1, %2
  %4 = lshr exact i64 %3, 2
  %xtraiter = and i64 %4, 3
  %5 = icmp ult i64 %3, 16
  br i1 %5, label %bb8.i.i.i.epil.preheader, label %bb5.i.i.i.new

bb5.i.i.i.new:                                    ; preds = %bb5.i.i.i
  %unroll_iter = and i64 %4, 4611686018427387900
  br label %bb8.i.i.i

bb8.i.i.i:                                        ; preds = %bb8.i.i.i, %bb5.i.i.i.new
  %i.sroa.0.0.i.i.i = phi i64 [ 0, %bb5.i.i.i.new ], [ %_27.i.i.i.3, %bb8.i.i.i ]
  %acc.sroa.0.0.i.i.i = phi i32 [ 0, %bb5.i.i.i.new ], [ %_4.0.i.i.i.i.i.3, %bb8.i.i.i ]
  %niter = phi i64 [ 0, %bb5.i.i.i.new ], [ %niter.next.3, %bb8.i.i.i ]
  %_45.i.i.i = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %self1.i.i.i.i.i.i = load i32, ptr %_45.i.i.i, align 4, !alias.scope !62, !noundef !5
  %_4.0.i.i.i.i.i.i = mul i32 %self1.i.i.i.i.i.i, 5
  %_4.0.i.i.i.i.i = add i32 %_4.0.i.i.i.i.i.i, %acc.sroa.0.0.i.i.i
  %6 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %_45.i.i.i.1 = getelementptr inbounds nuw i8, ptr %6, i64 4
  %self1.i.i.i.i.i.i.1 = load i32, ptr %_45.i.i.i.1, align 4, !alias.scope !62, !noundef !5
  %_4.0.i.i.i.i.i.i.1 = mul i32 %self1.i.i.i.i.i.i.1, 5
  %_4.0.i.i.i.i.i.1 = add i32 %_4.0.i.i.i.i.i.i.1, %_4.0.i.i.i.i.i
  %7 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %_45.i.i.i.2 = getelementptr inbounds nuw i8, ptr %7, i64 8
  %self1.i.i.i.i.i.i.2 = load i32, ptr %_45.i.i.i.2, align 4, !alias.scope !62, !noundef !5
  %_4.0.i.i.i.i.i.i.2 = mul i32 %self1.i.i.i.i.i.i.2, 5
  %_4.0.i.i.i.i.i.2 = add i32 %_4.0.i.i.i.i.i.i.2, %_4.0.i.i.i.i.i.1
  %8 = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i
  %_45.i.i.i.3 = getelementptr inbounds nuw i8, ptr %8, i64 12
  %self1.i.i.i.i.i.i.3 = load i32, ptr %_45.i.i.i.3, align 4, !alias.scope !62, !noundef !5
  %_4.0.i.i.i.i.i.i.3 = mul i32 %self1.i.i.i.i.i.i.3, 5
  %_4.0.i.i.i.i.i.3 = add i32 %_4.0.i.i.i.i.i.i.3, %_4.0.i.i.i.i.i.2
  %_27.i.i.i.3 = add nuw i64 %i.sroa.0.0.i.i.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit.loopexit.unr-lcssa, label %bb8.i.i.i

_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit.loopexit.unr-lcssa: ; preds = %bb8.i.i.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit, label %bb8.i.i.i.epil.preheader

bb8.i.i.i.epil.preheader:                         ; preds = %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit.loopexit.unr-lcssa, %bb5.i.i.i
  %i.sroa.0.0.i.i.i.epil.init = phi i64 [ 0, %bb5.i.i.i ], [ %_27.i.i.i.3, %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.i.i.epil.init = phi i32 [ 0, %bb5.i.i.i ], [ %_4.0.i.i.i.i.i.3, %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit.loopexit.unr-lcssa ]
  %lcmp.mod2 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod2)
  br label %bb8.i.i.i.epil

bb8.i.i.i.epil:                                   ; preds = %bb8.i.i.i.epil, %bb8.i.i.i.epil.preheader
  %i.sroa.0.0.i.i.i.epil = phi i64 [ %i.sroa.0.0.i.i.i.epil.init, %bb8.i.i.i.epil.preheader ], [ %_27.i.i.i.epil, %bb8.i.i.i.epil ]
  %acc.sroa.0.0.i.i.i.epil = phi i32 [ %acc.sroa.0.0.i.i.i.epil.init, %bb8.i.i.i.epil.preheader ], [ %_4.0.i.i.i.i.i.epil, %bb8.i.i.i.epil ]
  %epil.iter = phi i64 [ 0, %bb8.i.i.i.epil.preheader ], [ %epil.iter.next, %bb8.i.i.i.epil ]
  %_45.i.i.i.epil = getelementptr inbounds nuw [4 x i8], ptr %self.0, i64 %i.sroa.0.0.i.i.i.epil
  %self1.i.i.i.i.i.i.epil = load i32, ptr %_45.i.i.i.epil, align 4, !alias.scope !62, !noundef !5
  %_4.0.i.i.i.i.i.i.epil = mul i32 %self1.i.i.i.i.i.i.epil, 5
  %_4.0.i.i.i.i.i.epil = add i32 %_4.0.i.i.i.i.i.i.epil, %acc.sroa.0.0.i.i.i.epil
  %_27.i.i.i.epil = add nuw i64 %i.sroa.0.0.i.i.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit, label %bb8.i.i.i.epil, !llvm.loop !69

_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit: ; preds = %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit.loopexit.unr-lcssa, %bb8.i.i.i.epil, %start
  %_0.sroa.0.0.i.i.i = phi i32 [ 0, %start ], [ %_4.0.i.i.i.i.i.3, %_RINvXsy_NtNtNtCslK5Drzrx8K4_4core4iter6traits5accummNtB6_3Sum3sumINtNtNtBa_8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0EEB1Y_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.i.i.epil, %bb8.i.i.i.epil ]
  ret i32 %_0.sroa.0.0.i.i.i
}

; a::f
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read) uwtable
define noundef i32 @_RNvCs4oRDWwiVKk4_1a1f(ptr noalias nofree noundef nonnull readonly align 4 captures(none) %v.0, i64 noundef range(i64 0, 2305843009213693952) %v.1) unnamed_addr #2 personality ptr @rust_eh_personality !guid !70 {
start:
  %0 = icmp eq i64 %v.1, 0
  br i1 %0, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit, label %bb8.i.i.i.i.preheader

bb8.i.i.i.i.preheader:                            ; preds = %start
  %xtraiter = and i64 %v.1, 3
  %1 = icmp samesign ult i64 %v.1, 4
  br i1 %1, label %bb8.i.i.i.i.epil.preheader, label %bb8.i.i.i.i.preheader.new

bb8.i.i.i.i.preheader.new:                        ; preds = %bb8.i.i.i.i.preheader
  %unroll_iter = and i64 %v.1, 2305843009213693948
  br label %bb8.i.i.i.i

bb8.i.i.i.i:                                      ; preds = %bb8.i.i.i.i, %bb8.i.i.i.i.preheader.new
  %i.sroa.0.0.i.i.i.i = phi i64 [ 0, %bb8.i.i.i.i.preheader.new ], [ %_27.i.i.i.i.3, %bb8.i.i.i.i ]
  %acc.sroa.0.0.i.i.i.i = phi i32 [ 0, %bb8.i.i.i.i.preheader.new ], [ %_4.0.i.i.i.i.i.i.3, %bb8.i.i.i.i ]
  %niter = phi i64 [ 0, %bb8.i.i.i.i.preheader.new ], [ %niter.next.3, %bb8.i.i.i.i ]
  %_45.i.i.i.i = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %self1.i.i.i.i.i.i.i = load i32, ptr %_45.i.i.i.i, align 4, !alias.scope !71, !noundef !5
  %_4.0.i.i.i.i.i.i.i = mul i32 %self1.i.i.i.i.i.i.i, 3
  %_4.0.i.i.i.i.i.i = add i32 %_4.0.i.i.i.i.i.i.i, %acc.sroa.0.0.i.i.i.i
  %2 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %_45.i.i.i.i.1 = getelementptr inbounds nuw i8, ptr %2, i64 4
  %self1.i.i.i.i.i.i.i.1 = load i32, ptr %_45.i.i.i.i.1, align 4, !alias.scope !71, !noundef !5
  %_4.0.i.i.i.i.i.i.i.1 = mul i32 %self1.i.i.i.i.i.i.i.1, 3
  %_4.0.i.i.i.i.i.i.1 = add i32 %_4.0.i.i.i.i.i.i.i.1, %_4.0.i.i.i.i.i.i
  %3 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %_45.i.i.i.i.2 = getelementptr inbounds nuw i8, ptr %3, i64 8
  %self1.i.i.i.i.i.i.i.2 = load i32, ptr %_45.i.i.i.i.2, align 4, !alias.scope !71, !noundef !5
  %_4.0.i.i.i.i.i.i.i.2 = mul i32 %self1.i.i.i.i.i.i.i.2, 3
  %_4.0.i.i.i.i.i.i.2 = add i32 %_4.0.i.i.i.i.i.i.i.2, %_4.0.i.i.i.i.i.i.1
  %4 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %_45.i.i.i.i.3 = getelementptr inbounds nuw i8, ptr %4, i64 12
  %self1.i.i.i.i.i.i.i.3 = load i32, ptr %_45.i.i.i.i.3, align 4, !alias.scope !71, !noundef !5
  %_4.0.i.i.i.i.i.i.i.3 = mul i32 %self1.i.i.i.i.i.i.i.3, 3
  %_4.0.i.i.i.i.i.i.3 = add i32 %_4.0.i.i.i.i.i.i.i.3, %_4.0.i.i.i.i.i.i.2
  %_27.i.i.i.i.3 = add nuw nsw i64 %i.sroa.0.0.i.i.i.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa, label %bb8.i.i.i.i

_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa: ; preds = %bb8.i.i.i.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit, label %bb8.i.i.i.i.epil.preheader

bb8.i.i.i.i.epil.preheader:                       ; preds = %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa, %bb8.i.i.i.i.preheader
  %i.sroa.0.0.i.i.i.i.epil.init = phi i64 [ 0, %bb8.i.i.i.i.preheader ], [ %_27.i.i.i.i.3, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.i.i.i.epil.init = phi i32 [ 0, %bb8.i.i.i.i.preheader ], [ %_4.0.i.i.i.i.i.i.3, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa ]
  %lcmp.mod2 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod2)
  br label %bb8.i.i.i.i.epil

bb8.i.i.i.i.epil:                                 ; preds = %bb8.i.i.i.i.epil, %bb8.i.i.i.i.epil.preheader
  %i.sroa.0.0.i.i.i.i.epil = phi i64 [ %_27.i.i.i.i.epil, %bb8.i.i.i.i.epil ], [ %i.sroa.0.0.i.i.i.i.epil.init, %bb8.i.i.i.i.epil.preheader ]
  %acc.sroa.0.0.i.i.i.i.epil = phi i32 [ %_4.0.i.i.i.i.i.i.epil, %bb8.i.i.i.i.epil ], [ %acc.sroa.0.0.i.i.i.i.epil.init, %bb8.i.i.i.i.epil.preheader ]
  %epil.iter = phi i64 [ %epil.iter.next, %bb8.i.i.i.i.epil ], [ 0, %bb8.i.i.i.i.epil.preheader ]
  %_45.i.i.i.i.epil = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i.epil
  %self1.i.i.i.i.i.i.i.epil = load i32, ptr %_45.i.i.i.i.epil, align 4, !alias.scope !71, !noundef !5
  %_4.0.i.i.i.i.i.i.i.epil = mul i32 %self1.i.i.i.i.i.i.i.epil, 3
  %_4.0.i.i.i.i.i.i.epil = add i32 %_4.0.i.i.i.i.i.i.i.epil, %acc.sroa.0.0.i.i.i.i.epil
  %_27.i.i.i.i.epil = add nuw nsw i64 %i.sroa.0.0.i.i.i.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit, label %bb8.i.i.i.i.epil, !llvm.loop !78

_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit: ; preds = %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa, %bb8.i.i.i.i.epil, %start
  %_0.sroa.0.0.i.i.i.i = phi i32 [ 0, %start ], [ %_4.0.i.i.i.i.i.i.3, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1f0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.i.i.i.epil, %bb8.i.i.i.i.epil ]
  ret i32 %_0.sroa.0.0.i.i.i.i
}

; a::g
; Function Attrs: nofree norecurse nosync nounwind nonlazybind memory(argmem: read) uwtable
define noundef i32 @_RNvCs4oRDWwiVKk4_1a1g(ptr noalias nofree noundef nonnull readonly align 4 captures(none) %v.0, i64 noundef range(i64 0, 2305843009213693952) %v.1) unnamed_addr #2 personality ptr @rust_eh_personality !guid !79 {
start:
  %0 = icmp eq i64 %v.1, 0
  br i1 %0, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit, label %bb8.i.i.i.i.preheader

bb8.i.i.i.i.preheader:                            ; preds = %start
  %xtraiter = and i64 %v.1, 3
  %1 = icmp samesign ult i64 %v.1, 4
  br i1 %1, label %bb8.i.i.i.i.epil.preheader, label %bb8.i.i.i.i.preheader.new

bb8.i.i.i.i.preheader.new:                        ; preds = %bb8.i.i.i.i.preheader
  %unroll_iter = and i64 %v.1, 2305843009213693948
  br label %bb8.i.i.i.i

bb8.i.i.i.i:                                      ; preds = %bb8.i.i.i.i, %bb8.i.i.i.i.preheader.new
  %i.sroa.0.0.i.i.i.i = phi i64 [ 0, %bb8.i.i.i.i.preheader.new ], [ %_27.i.i.i.i.3, %bb8.i.i.i.i ]
  %acc.sroa.0.0.i.i.i.i = phi i32 [ 0, %bb8.i.i.i.i.preheader.new ], [ %_4.0.i.i.i.i.i.i.3, %bb8.i.i.i.i ]
  %niter = phi i64 [ 0, %bb8.i.i.i.i.preheader.new ], [ %niter.next.3, %bb8.i.i.i.i ]
  %_45.i.i.i.i = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %self1.i.i.i.i.i.i.i = load i32, ptr %_45.i.i.i.i, align 4, !alias.scope !80, !noundef !5
  %_4.0.i.i.i.i.i.i.i = mul i32 %self1.i.i.i.i.i.i.i, 5
  %_4.0.i.i.i.i.i.i = add i32 %_4.0.i.i.i.i.i.i.i, %acc.sroa.0.0.i.i.i.i
  %2 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %_45.i.i.i.i.1 = getelementptr inbounds nuw i8, ptr %2, i64 4
  %self1.i.i.i.i.i.i.i.1 = load i32, ptr %_45.i.i.i.i.1, align 4, !alias.scope !80, !noundef !5
  %_4.0.i.i.i.i.i.i.i.1 = mul i32 %self1.i.i.i.i.i.i.i.1, 5
  %_4.0.i.i.i.i.i.i.1 = add i32 %_4.0.i.i.i.i.i.i.i.1, %_4.0.i.i.i.i.i.i
  %3 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %_45.i.i.i.i.2 = getelementptr inbounds nuw i8, ptr %3, i64 8
  %self1.i.i.i.i.i.i.i.2 = load i32, ptr %_45.i.i.i.i.2, align 4, !alias.scope !80, !noundef !5
  %_4.0.i.i.i.i.i.i.i.2 = mul i32 %self1.i.i.i.i.i.i.i.2, 5
  %_4.0.i.i.i.i.i.i.2 = add i32 %_4.0.i.i.i.i.i.i.i.2, %_4.0.i.i.i.i.i.i.1
  %4 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i
  %_45.i.i.i.i.3 = getelementptr inbounds nuw i8, ptr %4, i64 12
  %self1.i.i.i.i.i.i.i.3 = load i32, ptr %_45.i.i.i.i.3, align 4, !alias.scope !80, !noundef !5
  %_4.0.i.i.i.i.i.i.i.3 = mul i32 %self1.i.i.i.i.i.i.i.3, 5
  %_4.0.i.i.i.i.i.i.3 = add i32 %_4.0.i.i.i.i.i.i.i.3, %_4.0.i.i.i.i.i.i.2
  %_27.i.i.i.i.3 = add nuw nsw i64 %i.sroa.0.0.i.i.i.i, 4
  %niter.next.3 = add i64 %niter, 4
  %niter.ncmp.3 = icmp eq i64 %niter.next.3, %unroll_iter
  br i1 %niter.ncmp.3, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa, label %bb8.i.i.i.i

_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa: ; preds = %bb8.i.i.i.i
  %lcmp.mod.not = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod.not, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit, label %bb8.i.i.i.i.epil.preheader

bb8.i.i.i.i.epil.preheader:                       ; preds = %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa, %bb8.i.i.i.i.preheader
  %i.sroa.0.0.i.i.i.i.epil.init = phi i64 [ 0, %bb8.i.i.i.i.preheader ], [ %_27.i.i.i.i.3, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.i.i.i.epil.init = phi i32 [ 0, %bb8.i.i.i.i.preheader ], [ %_4.0.i.i.i.i.i.i.3, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa ]
  %lcmp.mod3 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod3)
  br label %bb8.i.i.i.i.epil

bb8.i.i.i.i.epil:                                 ; preds = %bb8.i.i.i.i.epil, %bb8.i.i.i.i.epil.preheader
  %i.sroa.0.0.i.i.i.i.epil = phi i64 [ %_27.i.i.i.i.epil, %bb8.i.i.i.i.epil ], [ %i.sroa.0.0.i.i.i.i.epil.init, %bb8.i.i.i.i.epil.preheader ]
  %acc.sroa.0.0.i.i.i.i.epil = phi i32 [ %_4.0.i.i.i.i.i.i.epil, %bb8.i.i.i.i.epil ], [ %acc.sroa.0.0.i.i.i.i.epil.init, %bb8.i.i.i.i.epil.preheader ]
  %epil.iter = phi i64 [ %epil.iter.next, %bb8.i.i.i.i.epil ], [ 0, %bb8.i.i.i.i.epil.preheader ]
  %_45.i.i.i.i.epil = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i.epil
  %self1.i.i.i.i.i.i.i.epil = load i32, ptr %_45.i.i.i.i.epil, align 4, !alias.scope !80, !noundef !5
  %_4.0.i.i.i.i.i.i.i.epil = mul i32 %self1.i.i.i.i.i.i.i.epil, 5
  %_4.0.i.i.i.i.i.i.epil = add i32 %_4.0.i.i.i.i.i.i.i.epil, %acc.sroa.0.0.i.i.i.i.epil
  %_27.i.i.i.i.epil = add nuw nsw i64 %i.sroa.0.0.i.i.i.i.epil, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp.not = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp.not, label %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit, label %bb8.i.i.i.i.epil, !llvm.loop !87

_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit: ; preds = %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa, %bb8.i.i.i.i.epil, %start
  %_0.sroa.0.0.i.i.i.i = phi i32 [ 0, %start ], [ %_4.0.i.i.i.i.i.i.3, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.i.i.i.epil, %bb8.i.i.i.i.epil ]
  %5 = icmp eq i64 %v.1, 0
  br i1 %5, label %_RNvCs4oRDWwiVKk4_1a1f.exit, label %bb8.i.i.i.i.i.preheader

bb8.i.i.i.i.i.preheader:                          ; preds = %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit
  %xtraiter4 = and i64 %v.1, 3
  %6 = icmp samesign ult i64 %v.1, 4
  br i1 %6, label %bb8.i.i.i.i.i.epil.preheader, label %bb8.i.i.i.i.i.preheader.new

bb8.i.i.i.i.i.preheader.new:                      ; preds = %bb8.i.i.i.i.i.preheader
  %unroll_iter9 = and i64 %v.1, 2305843009213693948
  br label %bb8.i.i.i.i.i

bb8.i.i.i.i.i:                                    ; preds = %bb8.i.i.i.i.i, %bb8.i.i.i.i.i.preheader.new
  %i.sroa.0.0.i.i.i.i.i = phi i64 [ 0, %bb8.i.i.i.i.i.preheader.new ], [ %_27.i.i.i.i.i.3, %bb8.i.i.i.i.i ]
  %acc.sroa.0.0.i.i.i.i.i = phi i32 [ 0, %bb8.i.i.i.i.i.preheader.new ], [ %_4.0.i.i.i.i.i.i.i1.3, %bb8.i.i.i.i.i ]
  %niter10 = phi i64 [ 0, %bb8.i.i.i.i.i.preheader.new ], [ %niter10.next.3, %bb8.i.i.i.i.i ]
  %_45.i.i.i.i.i = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i.i
  %self1.i.i.i.i.i.i.i.i = load i32, ptr %_45.i.i.i.i.i, align 4, !alias.scope !88, !noundef !5
  %_4.0.i.i.i.i.i.i.i.i = mul i32 %self1.i.i.i.i.i.i.i.i, 3
  %_4.0.i.i.i.i.i.i.i1 = add i32 %_4.0.i.i.i.i.i.i.i.i, %acc.sroa.0.0.i.i.i.i.i
  %7 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i.i
  %_45.i.i.i.i.i.1 = getelementptr inbounds nuw i8, ptr %7, i64 4
  %self1.i.i.i.i.i.i.i.i.1 = load i32, ptr %_45.i.i.i.i.i.1, align 4, !alias.scope !88, !noundef !5
  %_4.0.i.i.i.i.i.i.i.i.1 = mul i32 %self1.i.i.i.i.i.i.i.i.1, 3
  %_4.0.i.i.i.i.i.i.i1.1 = add i32 %_4.0.i.i.i.i.i.i.i.i.1, %_4.0.i.i.i.i.i.i.i1
  %8 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i.i
  %_45.i.i.i.i.i.2 = getelementptr inbounds nuw i8, ptr %8, i64 8
  %self1.i.i.i.i.i.i.i.i.2 = load i32, ptr %_45.i.i.i.i.i.2, align 4, !alias.scope !88, !noundef !5
  %_4.0.i.i.i.i.i.i.i.i.2 = mul i32 %self1.i.i.i.i.i.i.i.i.2, 3
  %_4.0.i.i.i.i.i.i.i1.2 = add i32 %_4.0.i.i.i.i.i.i.i.i.2, %_4.0.i.i.i.i.i.i.i1.1
  %9 = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i.i
  %_45.i.i.i.i.i.3 = getelementptr inbounds nuw i8, ptr %9, i64 12
  %self1.i.i.i.i.i.i.i.i.3 = load i32, ptr %_45.i.i.i.i.i.3, align 4, !alias.scope !88, !noundef !5
  %_4.0.i.i.i.i.i.i.i.i.3 = mul i32 %self1.i.i.i.i.i.i.i.i.3, 3
  %_4.0.i.i.i.i.i.i.i1.3 = add i32 %_4.0.i.i.i.i.i.i.i.i.3, %_4.0.i.i.i.i.i.i.i1.2
  %_27.i.i.i.i.i.3 = add nuw nsw i64 %i.sroa.0.0.i.i.i.i.i, 4
  %niter10.next.3 = add i64 %niter10, 4
  %niter10.ncmp.3 = icmp eq i64 %niter10.next.3, %unroll_iter9
  br i1 %niter10.ncmp.3, label %_RNvCs4oRDWwiVKk4_1a1f.exit.loopexit.unr-lcssa, label %bb8.i.i.i.i.i

_RNvCs4oRDWwiVKk4_1a1f.exit.loopexit.unr-lcssa:   ; preds = %bb8.i.i.i.i.i
  %lcmp.mod6.not = icmp eq i64 %xtraiter4, 0
  br i1 %lcmp.mod6.not, label %_RNvCs4oRDWwiVKk4_1a1f.exit, label %bb8.i.i.i.i.i.epil.preheader

bb8.i.i.i.i.i.epil.preheader:                     ; preds = %_RNvCs4oRDWwiVKk4_1a1f.exit.loopexit.unr-lcssa, %bb8.i.i.i.i.i.preheader
  %i.sroa.0.0.i.i.i.i.i.epil.init = phi i64 [ 0, %bb8.i.i.i.i.i.preheader ], [ %_27.i.i.i.i.i.3, %_RNvCs4oRDWwiVKk4_1a1f.exit.loopexit.unr-lcssa ]
  %acc.sroa.0.0.i.i.i.i.i.epil.init = phi i32 [ 0, %bb8.i.i.i.i.i.preheader ], [ %_4.0.i.i.i.i.i.i.i1.3, %_RNvCs4oRDWwiVKk4_1a1f.exit.loopexit.unr-lcssa ]
  %lcmp.mod8 = icmp ne i64 %xtraiter4, 0
  tail call void @llvm.assume(i1 %lcmp.mod8)
  br label %bb8.i.i.i.i.i.epil

bb8.i.i.i.i.i.epil:                               ; preds = %bb8.i.i.i.i.i.epil, %bb8.i.i.i.i.i.epil.preheader
  %i.sroa.0.0.i.i.i.i.i.epil = phi i64 [ %_27.i.i.i.i.i.epil, %bb8.i.i.i.i.i.epil ], [ %i.sroa.0.0.i.i.i.i.i.epil.init, %bb8.i.i.i.i.i.epil.preheader ]
  %acc.sroa.0.0.i.i.i.i.i.epil = phi i32 [ %_4.0.i.i.i.i.i.i.i1.epil, %bb8.i.i.i.i.i.epil ], [ %acc.sroa.0.0.i.i.i.i.i.epil.init, %bb8.i.i.i.i.i.epil.preheader ]
  %epil.iter5 = phi i64 [ %epil.iter5.next, %bb8.i.i.i.i.i.epil ], [ 0, %bb8.i.i.i.i.i.epil.preheader ]
  %_45.i.i.i.i.i.epil = getelementptr inbounds nuw [4 x i8], ptr %v.0, i64 %i.sroa.0.0.i.i.i.i.i.epil
  %self1.i.i.i.i.i.i.i.i.epil = load i32, ptr %_45.i.i.i.i.i.epil, align 4, !alias.scope !88, !noundef !5
  %_4.0.i.i.i.i.i.i.i.i.epil = mul i32 %self1.i.i.i.i.i.i.i.i.epil, 3
  %_4.0.i.i.i.i.i.i.i1.epil = add i32 %_4.0.i.i.i.i.i.i.i.i.epil, %acc.sroa.0.0.i.i.i.i.i.epil
  %_27.i.i.i.i.i.epil = add nuw nsw i64 %i.sroa.0.0.i.i.i.i.i.epil, 1
  %epil.iter5.next = add i64 %epil.iter5, 1
  %epil.iter5.cmp.not = icmp eq i64 %epil.iter5.next, %xtraiter4
  br i1 %epil.iter5.cmp.not, label %_RNvCs4oRDWwiVKk4_1a1f.exit, label %bb8.i.i.i.i.i.epil, !llvm.loop !97

_RNvCs4oRDWwiVKk4_1a1f.exit:                      ; preds = %_RNvCs4oRDWwiVKk4_1a1f.exit.loopexit.unr-lcssa, %bb8.i.i.i.i.i.epil, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit
  %_0.sroa.0.0.i.i.i.i.i = phi i32 [ 0, %_RINvYINtNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map3MapINtNtNtBc_5slice4iter4ItermENCNvCs4oRDWwiVKk4_1a1g0ENtNtNtBa_6traits8iterator8Iterator3summEB1n_.exit ], [ %_4.0.i.i.i.i.i.i.i1.3, %_RNvCs4oRDWwiVKk4_1a1f.exit.loopexit.unr-lcssa ], [ %_4.0.i.i.i.i.i.i.i1.epil, %bb8.i.i.i.i.i.epil ]
  %_0 = add i32 %_0.sroa.0.0.i.i.i.i.i, %_0.sroa.0.0.i.i.i.i
  ret i32 %_0
}

; a::h
; Function Attrs: nonlazybind uwtable
define noundef i32 @_RNvCs4oRDWwiVKk4_1a1h(ptr noundef nonnull %0, ptr noalias nofree noundef readonly align 8 captures(address, read_provenance) dereferenceable(48) %1) unnamed_addr #0 personality ptr @rust_eh_personality !guid !98 {
start:
  %b = alloca [16 x i8], align 8
  store ptr %0, ptr %b, align 8
  %2 = getelementptr inbounds nuw i8, ptr %b, i64 8
  store ptr %1, ptr %2, align 8
  %3 = getelementptr inbounds nuw i8, ptr %1, i64 40
  %4 = load ptr, ptr %3, align 8, !invariant.load !5, !noalias !99, !nonnull !5
  %_0.i1 = invoke noundef i32 %4(ptr noundef nonnull %0, i32 noundef 1)
          to label %bb1 unwind label %cleanup, !inline_history !102

cleanup:                                          ; preds = %bb1, %start
  %5 = landingpad { ptr, i32 }
          cleanup
; invoke core::ptr::drop_glue::<alloc::boxed::Box<dyn core::ops::function::Fn<(u32,), Output = u32>>>
  invoke fastcc void @_RINvNtCslK5Drzrx8K4_4core3ptr9drop_glueINtNtCsdf08ABbzq28_5alloc5boxed3BoxDINtNtNtB4_3ops8function2FnTmEEp6OutputmEL_EECs4oRDWwiVKk4_1a(ptr noalias nofree noundef align 8 dereferenceable(16) %b) #9
          to label %common.resume unwind label %terminate

bb1:                                              ; preds = %start
  %_0.i4 = invoke noundef i32 %4(ptr noundef nonnull %0, i32 noundef 2)
          to label %bb2 unwind label %cleanup, !inline_history !102

bb2:                                              ; preds = %bb1
  %6 = load ptr, ptr %1, align 8, !invariant.load !5, !noalias !103
  %.not.i = icmp eq ptr %6, null
  br i1 %.not.i, label %bb3.i, label %is_not_null.i

is_not_null.i:                                    ; preds = %bb2
  invoke void %6(ptr noundef nonnull %0)
          to label %bb3.i unwind label %cleanup.i, !noalias !103

bb3.i:                                            ; preds = %is_not_null.i, %bb2
  %7 = getelementptr inbounds nuw i8, ptr %1, i64 8
  %size.i.i = load i64, ptr %7, align 8, !range !10, !invariant.load !5, !noalias !106
  %8 = icmp eq i64 %size.i.i, 0
  br i1 %8, label %_RINvNtCslK5Drzrx8K4_4core3ptr9drop_glueINtNtCsdf08ABbzq28_5alloc5boxed3BoxDINtNtNtB4_3ops8function2FnTmEEp6OutputmEL_EECs4oRDWwiVKk4_1a.exit, label %bb1.i.i

bb1.i.i:                                          ; preds = %bb3.i
  %9 = getelementptr inbounds nuw i8, ptr %1, i64 16
  %align.i.i = load i64, ptr %9, align 8, !range !11, !invariant.load !5, !noalias !106
; call __rustc::__rust_dealloc
  tail call void @_RNvCsjMmYDGKcCRn_7___rustc14___rust_dealloc(ptr noundef nonnull %0, i64 noundef range(i64 1, -9223372036854775808) %size.i.i, i64 noundef range(i64 1, 536870913) %align.i.i) #8, !noalias !106
  br label %_RINvNtCslK5Drzrx8K4_4core3ptr9drop_glueINtNtCsdf08ABbzq28_5alloc5boxed3BoxDINtNtNtB4_3ops8function2FnTmEEp6OutputmEL_EECs4oRDWwiVKk4_1a.exit

cleanup.i:                                        ; preds = %is_not_null.i
  %10 = landingpad { ptr, i32 }
          cleanup
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 8
  %size.i2.i = load i64, ptr %11, align 8, !range !10, !invariant.load !5, !noalias !109
  %12 = icmp eq i64 %size.i2.i, 0
  br i1 %12, label %common.resume, label %bb1.i3.i

bb1.i3.i:                                         ; preds = %cleanup.i
  %13 = getelementptr inbounds nuw i8, ptr %1, i64 16
  %align.i4.i = load i64, ptr %13, align 8, !range !11, !invariant.load !5, !noalias !109
; call __rustc::__rust_dealloc
  tail call void @_RNvCsjMmYDGKcCRn_7___rustc14___rust_dealloc(ptr noundef nonnull %0, i64 noundef range(i64 1, -9223372036854775808) %size.i2.i, i64 noundef range(i64 1, 536870913) %align.i4.i) #8, !noalias !109
  br label %common.resume

common.resume:                                    ; preds = %cleanup, %cleanup.i, %bb1.i3.i
  %common.resume.op = phi { ptr, i32 } [ %10, %cleanup.i ], [ %10, %bb1.i3.i ], [ %5, %cleanup ]
  resume { ptr, i32 } %common.resume.op

_RINvNtCslK5Drzrx8K4_4core3ptr9drop_glueINtNtCsdf08ABbzq28_5alloc5boxed3BoxDINtNtNtB4_3ops8function2FnTmEEp6OutputmEL_EECs4oRDWwiVKk4_1a.exit: ; preds = %bb3.i, %bb1.i.i
  %_0 = add i32 %_0.i4, %_0.i1
  ret i32 %_0

terminate:                                        ; preds = %cleanup
  %14 = landingpad { ptr, i32 }
          filter [0 x ptr] zeroinitializer
; call core::panicking::panic_in_cleanup
  tail call void @_RNvNtCslK5Drzrx8K4_4core9panicking16panic_in_cleanup() #10
  unreachable
}

; <alloc::boxed::Box<dyn core::ops::function::Fn<(u32,), Output = u32>> as core::ops::function::Fn<(u32,)>>::call
; Function Attrs: nonlazybind uwtable
define noundef i32 @_RNvXsv_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_EIBJ_B1o_E4callCs4oRDWwiVKk4_1a(ptr noalias nofree noundef readonly align 8 captures(none) dereferenceable(16) %self, i32 noundef %0) unnamed_addr #0 !guid !112 {
start:
  %_5.0 = load ptr, ptr %self, align 8, !nonnull !5, !noundef !5
  %1 = getelementptr inbounds nuw i8, ptr %self, i64 8
  %_5.1 = load ptr, ptr %1, align 8, !nonnull !5, !align !6, !noundef !5
  %2 = getelementptr inbounds nuw i8, ptr %_5.1, i64 40
  %3 = load ptr, ptr %2, align 8, !invariant.load !5, !nonnull !5
  %_0 = tail call noundef i32 %3(ptr noundef nonnull %_5.0, i32 noundef %0)
  ret i32 %_0
}

; Function Attrs: nounwind nonlazybind uwtable
declare noundef range(i32 0, 10) i32 @rust_eh_personality(i32 noundef, i32 noundef, i64 noundef, ptr noundef, ptr noundef) unnamed_addr #3

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write)
declare void @llvm.assume(i1 noundef) #4

; core::panicking::panic_in_cleanup
; Function Attrs: cold minsize noinline noreturn nounwind nonlazybind optsize uwtable
declare void @_RNvNtCslK5Drzrx8K4_4core9panicking16panic_in_cleanup() unnamed_addr #5

; __rustc::__rust_dealloc
; Function Attrs: nounwind nonlazybind allockind("free") uwtable
declare void @_RNvCsjMmYDGKcCRn_7___rustc14___rust_dealloc(ptr allocptr noundef nonnull captures(address), i64 noundef, i64 noundef range(i64 1, -9223372036854775807)) unnamed_addr #6

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: readwrite)
declare void @llvm.experimental.noalias.scope.decl(metadata) #7

attributes #0 = { nonlazybind uwtable "probe-stack"="inline-asm" "target-cpu"="x86-64" "target-features"="+retpoline-indirect-branches,+retpoline-indirect-calls" }
attributes #1 = { nofree norecurse nosync nounwind nonlazybind memory(argmem: read, inaccessiblemem: write) uwtable "probe-stack"="inline-asm" "target-cpu"="x86-64" "target-features"="+retpoline-indirect-branches,+retpoline-indirect-calls" }
attributes #2 = { nofree norecurse nosync nounwind nonlazybind memory(argmem: read) uwtable "probe-stack"="inline-asm" "target-cpu"="x86-64" "target-features"="+retpoline-indirect-branches,+retpoline-indirect-calls" }
attributes #3 = { nounwind nonlazybind uwtable "probe-stack"="inline-asm" "target-cpu"="x86-64" "target-features"="+retpoline-indirect-branches,+retpoline-indirect-calls" }
attributes #4 = { mustprogress nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write) }
attributes #5 = { cold minsize noinline noreturn nounwind nonlazybind optsize uwtable "probe-stack"="inline-asm" "target-cpu"="x86-64" "target-features"="+retpoline-indirect-branches,+retpoline-indirect-calls" }
attributes #6 = { nounwind nonlazybind allockind("free") uwtable "alloc-family"="__rust_alloc" "probe-stack"="inline-asm" "target-cpu"="x86-64" "target-features"="+retpoline-indirect-branches,+retpoline-indirect-calls" }
attributes #7 = { nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: readwrite) }
attributes #8 = { nounwind }
attributes #9 = { cold }
attributes #10 = { cold noreturn nounwind }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 2, !"RtLibUseGOT", i32 1}
!2 = !{i32 7, !"uwtable", i32 2}
!3 = !{!"rustc version 1.101.0-nightly (ea137335b 2026-10-05)"}
!4 = !{i64 7465511351322191071}
!5 = !{}
!6 = !{i64 8}
!7 = !{!8}
!8 = distinct !{!8, !9, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a: %self"}
!9 = distinct !{!9, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a"}
!10 = !{i64 0, i64 -9223372036854775808}
!11 = !{i64 1, i64 536870913}
!12 = !{!13}
!13 = distinct !{!13, !14, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a: %self"}
!14 = distinct !{!14, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a"}
!15 = !{i64 -6662949334666188821}
!16 = !{!17, !19, !21}
!17 = distinct !{!17, !18, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!18 = distinct !{!18, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!19 = distinct !{!19, !20, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_: %x"}
!20 = distinct !{!20, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_"}
!21 = distinct !{!21, !22, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!22 = distinct !{!22, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!23 = distinct !{!23, !24}
!24 = !{!"llvm.loop.unroll.disable"}
!25 = !{i64 2100357767974896583}
!26 = !{!27, !29, !31}
!27 = distinct !{!27, !28, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!28 = distinct !{!28, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!29 = distinct !{!29, !30, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_: %x"}
!30 = distinct !{!30, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_"}
!31 = distinct !{!31, !32, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!32 = distinct !{!32, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!33 = distinct !{!33, !24}
!34 = !{i64 -5701431404630276692}
!35 = !{!36, !38, !40}
!36 = distinct !{!36, !37, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!37 = distinct !{!37, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!38 = distinct !{!38, !39, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_: %x"}
!39 = distinct !{!39, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_"}
!40 = distinct !{!40, !41, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!41 = distinct !{!41, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!42 = distinct !{!42, !24}
!43 = !{i64 -400067957793474969}
!44 = !{!45, !47, !49}
!45 = distinct !{!45, !46, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!46 = distinct !{!46, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!47 = distinct !{!47, !48, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_: %x"}
!48 = distinct !{!48, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_"}
!49 = distinct !{!49, !50, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!50 = distinct !{!50, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!51 = distinct !{!51, !24}
!52 = !{i64 -5648971341251397048}
!53 = !{!54, !56, !58}
!54 = distinct !{!54, !55, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!55 = distinct !{!55, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!56 = distinct !{!56, !57, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_: %x"}
!57 = distinct !{!57, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_"}
!58 = distinct !{!58, !59, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!59 = distinct !{!59, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!60 = distinct !{!60, !24}
!61 = !{i64 4076862687305757696}
!62 = !{!63, !65, !67}
!63 = distinct !{!63, !64, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!64 = distinct !{!64, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!65 = distinct !{!65, !66, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_: %x"}
!66 = distinct !{!66, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_"}
!67 = distinct !{!67, !68, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!68 = distinct !{!68, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!69 = distinct !{!69, !24}
!70 = !{i64 6299528273993967909}
!71 = !{!72, !74, !76}
!72 = distinct !{!72, !73, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!73 = distinct !{!73, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!74 = distinct !{!74, !75, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_: %x"}
!75 = distinct !{!75, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_"}
!76 = distinct !{!76, !77, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!77 = distinct !{!77, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!78 = distinct !{!78, !24}
!79 = !{i64 354688157513307938}
!80 = !{!81, !83, !85}
!81 = distinct !{!81, !82, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!82 = distinct !{!82, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!83 = distinct !{!83, !84, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_: %x"}
!84 = distinct !{!84, !"_RNCNvCs4oRDWwiVKk4_1a1g0B3_"}
!85 = distinct !{!85, !86, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!86 = distinct !{!86, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1g0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!87 = distinct !{!87, !24}
!88 = !{!89, !91, !93, !95}
!89 = distinct !{!89, !90, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul: %self"}
!90 = distinct !{!90, !"_RNvXs2r_NtNtCslK5Drzrx8K4_4core3ops5arithRmINtB6_3MulmE3mul"}
!91 = distinct !{!91, !92, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_: %x"}
!92 = distinct !{!92, !"_RNCNvCs4oRDWwiVKk4_1a1f0B3_"}
!93 = distinct !{!93, !94, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_: %elt"}
!94 = distinct !{!94, !"_RNCINvNtNtNtCslK5Drzrx8K4_4core4iter8adapters3map8map_foldRmmmNCNvCs4oRDWwiVKk4_1a1f0NCINvXsy_NtNtB8_6traits5accummNtB1u_3Sum3sumINtB4_3MapINtNtNtBa_5slice4iter4ItermEBY_EE0E0B12_"}
!95 = distinct !{!95, !96, !"_RNvCs4oRDWwiVKk4_1a1f: %v.0"}
!96 = distinct !{!96, !"_RNvCs4oRDWwiVKk4_1a1f"}
!97 = distinct !{!97, !24}
!98 = !{i64 7213187859578460101}
!99 = !{!100}
!100 = distinct !{!100, !101, !"_RNvXsv_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_EIBJ_B1o_E4callCs4oRDWwiVKk4_1a: %self"}
!101 = distinct !{!101, !"_RNvXsv_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_EIBJ_B1o_E4callCs4oRDWwiVKk4_1a"}
!102 = !{ptr @_RNvXsv_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_EIBJ_B1o_E4callCs4oRDWwiVKk4_1a}
!103 = !{!104}
!104 = distinct !{!104, !105, !"_RINvNtCslK5Drzrx8K4_4core3ptr9drop_glueINtNtCsdf08ABbzq28_5alloc5boxed3BoxDINtNtNtB4_3ops8function2FnTmEEp6OutputmEL_EECs4oRDWwiVKk4_1a: %_1"}
!105 = distinct !{!105, !"_RINvNtCslK5Drzrx8K4_4core3ptr9drop_glueINtNtCsdf08ABbzq28_5alloc5boxed3BoxDINtNtNtB4_3ops8function2FnTmEEp6OutputmEL_EECs4oRDWwiVKk4_1a"}
!106 = !{!107, !104}
!107 = distinct !{!107, !108, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a: %self"}
!108 = distinct !{!108, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a"}
!109 = !{!110, !104}
!110 = distinct !{!110, !111, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a: %self"}
!111 = distinct !{!111, !"_RNvXs8_NtCsdf08ABbzq28_5alloc5boxedINtB5_3BoxDINtNtNtCslK5Drzrx8K4_4core3ops8function2FnTmEEp6OutputmEL_ENtNtBN_4drop4Drop4dropCs4oRDWwiVKk4_1a"}
!112 = !{i64 1724830960605013563}
