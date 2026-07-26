; Reduced from a Rust program built for a bare-metal mips64 target (N64).
; Reproduces with:
;   llc -mtriple=mips64-unknown-unknown -mcpu=mips3 -O3 mips-sll64_64.ll -o out.s
; The i64 loop increments derived via `ashr exact i64 %x, 32` (sign-extend-from-32) lose
; their truncation: SLL64_64 is deleted during register allocation because it is marked
; isMoveReg, so the untruncated 64-bit value is added instead.
target datalayout = "E-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128"
target triple = "mips64-unknown-unknown"

define internal fastcc void @_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp3runNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest(ptr noalias nofree noundef nonnull align 8 captures(none) dereferenceable(40) %self, ptr noalias nofree noundef nonnull readonly align 8 captures(none) %stream.0, i64 noundef range(i64 0, 1152921504606846976) %stream.1, ptr noalias nofree noundef nonnull readonly align 8 captures(none) dereferenceable(40) %mem) unnamed_addr #1 {
bb3.lr.ph:                                        ; preds = %start
  %self.promoted = load i64, ptr %self, align 8
  %0 = getelementptr inbounds nuw i8, ptr %self, i64 8
  %1 = getelementptr inbounds nuw i8, ptr %self, i64 16
  %2 = getelementptr inbounds nuw i8, ptr %self, i64 24
  %3 = getelementptr inbounds nuw i8, ptr %mem, i64 32
  %_10.i.i.i14.i = load i32, ptr %3, align 8
  %4 = getelementptr inbounds nuw i8, ptr %mem, i64 8
  %_7.1.i.i.i17.i = load i64, ptr %4, align 8
  %5 = getelementptr inbounds nuw i8, ptr %self, i64 32
  %6 = getelementptr inbounds nuw i8, ptr %self, i64 28
  %.promoted = load i64, ptr %0, align 8
  %.promoted83 = load i64, ptr %1, align 8
  %.promoted86 = load i32, ptr %2, align 8
  %.promoted89 = load i32, ptr %5, align 8
  %.promoted90 = load i32, ptr %6, align 4
  br label %bb3
bb8:                                              ; preds = %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, %start, %bb4
  ret void
bb3:                                              ; preds = %bb3.lr.ph, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
  %blend.i92 = phi i32 [ %.promoted90, %bb3.lr.ph ], [ %blend.i91, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit ]
  %7 = phi i32 [ %.promoted89, %bb3.lr.ph ], [ %151, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit ]
  %state.val12.i88 = phi i32 [ %.promoted86, %bb3.lr.ph ], [ %state.val12.i87, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit ]
  %state.val.i85 = phi i64 [ %.promoted83, %bb3.lr.ph ], [ %state.val.i84, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit ]
  %_37.i82 = phi i64 [ %.promoted, %bb3.lr.ph ], [ %_37.i81, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit ]
  %index.sroa.0.080 = phi i64 [ 0, %bb3.lr.ph ], [ %_14, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit ]
  %_107779 = phi i64 [ %self.promoted, %bb3.lr.ph ], [ %_1076, %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit ]
  %8 = getelementptr inbounds nuw i64, ptr %stream.0, i64 %index.sroa.0.080
  %_10 = load i64, ptr %8, align 8
  %_24 = lshr i64 %_10, 56
  %9 = trunc nuw i64 %_24 to i8
  %_9 = and i8 %9, 63
  switch i8 %_9, label %bb9 [
    i8 9, label %bb18
  ]
bb19:                                             ; preds = %bb3
  br label %bb9
bb18:                                             ; preds = %bb3
  br label %bb9
bb17:                                             ; preds = %bb3, %bb3
  br label %bb9
bb16:                                             ; preds = %bb3, %bb3
  br label %bb9
bb13:                                             ; preds = %bb3
  br label %bb9
bb12:                                             ; preds = %bb3
  br label %bb9
bb11:                                             ; preds = %bb3, %bb3
  br label %bb9
bb9:                                              ; preds = %bb3, %bb11, %bb12, %bb13, %bb16, %bb17, %bb18, %bb19
  %_42.not.i.i = phi i1 [ true, %bb11 ], [ false, %bb19 ], [ false, %bb18 ], [ false, %bb17 ], [ false, %bb16 ], [ false, %bb3 ], [ false, %bb12 ], [ false, %bb13 ]
  %_52.i.i = phi i1 [ false, %bb11 ], [ true, %bb19 ], [ true, %bb18 ], [ true, %bb17 ], [ true, %bb16 ], [ false, %bb3 ], [ true, %bb12 ], [ true, %bb13 ]
  %length.sroa.0.0 = phi i64 [ 2, %bb11 ], [ 4, %bb19 ], [ 6, %bb18 ], [ 12, %bb17 ], [ 14, %bb16 ], [ 1, %bb3 ], [ 22, %bb12 ], [ 20, %bb13 ]
  %_14 = add nuw nsw i64 %length.sroa.0.0, %index.sroa.0.080
  %_13 = icmp samesign ugt i64 %_14, %stream.1
  br i1 %_13, label %bb4, label %bb20
bb4:                                              ; preds = %bb9
  br label %bb8
bb20:                                             ; preds = %bb9
  switch i8 %_9, label %bb2.i [
    i8 0, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
    i8 38, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
    i8 39, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
    i8 40, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
    i8 41, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
    i8 8, label %bb4.i
  ]
bb2.i:                                            ; preds = %bb20
  %11 = add i32 %7, 1
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb9.i:                                            ; preds = %bb20
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb8.i:                                            ; preds = %bb20
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb7.i:                                            ; preds = %bb20
  %12 = trunc i64 %_10 to i32
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb6.i:                                            ; preds = %bb20
  %13 = trunc i64 %_10 to i32
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb5.i:                                            ; preds = %bb20
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb4.i:                                            ; preds = %bb20
  %14 = and i64 %_107779, 13510798882111488
  %15 = icmp eq i64 %14, 0
  br i1 %15, label %bb33.i, label %bb34.i
bb3.i:                                            ; preds = %bb20
  %_18.i = lshr i64 %_107779, 52
  %_17.i = and i64 %_18.i, 3
  switch i64 %_17.i, label %default.unreachable [
  ]
default.unreachable:                              ; preds = %bb3.i
  unreachable
bb33.i:                                           ; preds = %bb4.i
  %16 = and i64 %_107779, 3435922176
  %or.cond30.i.i = icmp eq i64 %16, 2152464896
  br i1 %or.cond30.i.i, label %bb6.i.i, label %bb12.i
bb6.i.i:                                          ; preds = %bb33.i
  %_148.i.i = lshr i64 %state.val.i85, 51
  %17 = trunc nuw nsw i64 %_148.i.i to i32
  %size.i.i = and i32 %17, 3
  %18 = and i32 %17, 2
  %switch.not.i.i = icmp eq i32 %18, 0
  br i1 %switch.not.i.i, label %bb12.i, label %bb10.i.i
bb10.i.i:                                         ; preds = %bb6.i.i
  %19 = and i64 %_10, 36028797018963968
  %right_major.not.i.i = icmp eq i64 %19, 0
  %sh.diff.i.i = lshr i64 %_10, 14
  %tr.sh.diff.i.i = trunc i64 %sh.diff.i.i to i32
  %20 = trunc i64 %_10 to i32
  %21 = shl i32 %20, 2
  %_24.i.i = shl i32 %20, 18
  %22 = ashr exact i32 %_24.i.i, 18
  %23 = getelementptr inbounds nuw i8, ptr %8, i64 8
  %_31.i.i = load i64, ptr %23, align 8
  %24 = ashr i64 %_31.i.i, 30
  %xl.i.i = and i64 %24, -4
  br i1 %_42.not.i.i, label %panic6.i.i, label %bb16.i.i
panic4.i.i:                                       ; preds = %bb10.i.i
  unreachable
bb16.i.i:                                         ; preds = %bb14.i.i
  %25 = getelementptr inbounds nuw i8, ptr %8, i64 16
  %_41.i.i = load i64, ptr %25, align 8
  %_44.i.i = shl i64 %_41.i.i, 32
  %dh.i.i = ashr exact i64 %_44.i.i, 32
  br i1 %_52.i.i, label %bb18.i.i, label %panic8.i.i
panic6.i.i:                                       ; preds = %bb14.i.i
  unreachable
bb18.i.i:                                         ; preds = %bb16.i.i
  %26 = ashr i64 %_41.i.i, 30
  %27 = and i64 %26, -4
  %28 = getelementptr inbounds nuw i8, ptr %8, i64 24
  %_51.i.i = load i64, ptr %28, align 8
  %29 = ashr i64 %_51.i.i, 30
  %30 = and i64 %29, -4
  %_158.i.i = lshr i64 %_37.i82, 44
  %sc_left.i.i = and i64 %_158.i.i, 4095
  %_162.i.i = lshr i64 %_37.i82, 32
  %31 = trunc nuw i64 %_162.i.i to i32
  %sc_top.i.i = and i32 %31, 4095
  %32 = lshr i64 %_37.i82, 12
  %sc_right.i.i = and i64 %32, 4095
  %33 = trunc i64 %_37.i82 to i32
  %sc_bottom.i.i = and i32 %33, 4095
  %_57.i.i = add nuw nsw i64 %sc_left.i.i, 3
  %first_pixel_x.i.i = lshr i64 %_57.i.i, 2
  %_60.i.i = add nuw nsw i32 %sc_top.i.i, 3
  %first_pixel_y.i.i = lshr i32 %_60.i.i, 2
  %34 = trunc i64 %state.val.i85 to i32
  %base.i.i = and i32 %34, 67108863
  %_170.i.i = lshr i64 %state.val.i85, 32
  %35 = trunc nuw i64 %_170.i.i to i32
  %_70.i.i = and i32 %35, 1023
  %stride_pixels.i.i = add nuw nsw i32 %_70.i.i, 1
  %40 = add nsw i64 %sc_right.i.i, -1
  %41 = icmp eq i32 %size.i.i, 2
  %minor_inc.sroa.0.0.in.us.i.i = shl i64 %_51.i.i, 32
  %minor_inc.sroa.0.0.us.i.i = ashr exact i64 %minor_inc.sroa.0.0.in.us.i.i, 32
  %y_target.sroa.0.0.us.i.i = ashr i32 %21, 18
  %_89.not50.us.i.i = icmp slt i32 %22, %y_target.sroa.0.0.us.i.i
  br i1 %41, label %bb20.outer.us.preheader.i.i, label %bb20.outer.preheader.i.i
bb20.outer.preheader.i.i:                         ; preds = %bb18.i.i
  br i1 %_89.not50.us.i.i, label %bb27.preheader.i.i, label %bb24.i.i
bb20.outer.us.preheader.i.i:                      ; preds = %bb18.i.i
  br i1 %_89.not50.us.i.i, label %bb27.us.us.preheader.i.i, label %bb24.us.i.i
bb27.us.us.preheader.i.i:                         ; preds = %bb20.outer.us.preheader.i.i
  %smax77.i.i = tail call i32 @llvm.smax.i32(i32 %22, i32 %sc_bottom.i.i)
  %_117.us.us.us.i.i = sub i32 %base.i.i, %_10.i.i.i14.i
  br label %bb27.us.us.i.i
bb24.us.i.i:                                      ; preds = %bb47.us.us.i.i, %bb20.outer.us.preheader.i.i
  %xh.sroa.0.0.lcssa.us.i.i = phi i64 [ %27, %bb20.outer.us.preheader.i.i ], [ %54, %bb47.us.us.i.i ]
  %yh.sroa.0.0.lcssa.us.i.i = phi i32 [ %22, %bb20.outer.us.preheader.i.i ], [ %y_target.sroa.0.0.us.i.i, %bb47.us.us.i.i ]
  %minor_inc.sroa.0.0.in.us.1.i.i = shl i64 %_31.i.i, 32
  %minor_inc.sroa.0.0.us.1.i.i = ashr exact i64 %minor_inc.sroa.0.0.in.us.1.i.i, 32
  %y_target.sroa.0.0.us.1.i.i = ashr i32 %tr.sh.diff.i.i, 18
  %_89.not50.us.1.i.i = icmp slt i32 %yh.sroa.0.0.lcssa.us.i.i, %y_target.sroa.0.0.us.1.i.i
  br i1 %_89.not50.us.1.i.i, label %bb27.us.us.preheader.1.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb27.us.us.preheader.1.i.i:                       ; preds = %bb24.us.i.i
  %smax77.1.i.i = tail call i32 @llvm.smax.i32(i32 %yh.sroa.0.0.lcssa.us.i.i, i32 %sc_bottom.i.i)
  %_117.us.us.us.1.i.i = sub i32 %base.i.i, %_10.i.i.i14.i
  br label %bb27.us.us.1.i.i
bb27.us.us.1.i.i:                                 ; preds = %bb47.us.us.1.i.i, %bb27.us.us.preheader.1.i.i
  %yh.sroa.0.053.us.us.1.i.i = phi i32 [ %51, %bb47.us.us.1.i.i ], [ %yh.sroa.0.0.lcssa.us.i.i, %bb27.us.us.preheader.1.i.i ]
  %xh.sroa.0.052.us.us.1.i.i = phi i64 [ %49, %bb47.us.us.1.i.i ], [ %xh.sroa.0.0.lcssa.us.i.i, %bb27.us.us.preheader.1.i.i ]
  %xm.sroa.0.051.us.us.1.i.i = phi i64 [ %50, %bb47.us.us.1.i.i ], [ %xl.i.i, %bb27.us.us.preheader.1.i.i ]
  %exitcond78.1.not.i.i = icmp eq i32 %yh.sroa.0.053.us.us.1.i.i, %smax77.1.i.i
  br i1 %exitcond78.1.not.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb29.us.us.1.i.i
bb29.us.us.1.i.i:                                 ; preds = %bb27.us.us.1.i.i
  %_94.us.us.1.i.i = and i32 %yh.sroa.0.053.us.us.1.i.i, 3
  %42 = icmp eq i32 %_94.us.us.1.i.i, 0
  br i1 %42, label %bb30.us.us.1.i.i, label %bb47.us.us.1.i.i
bb30.us.us.1.i.i:                                 ; preds = %bb29.us.us.1.i.i
  %_97.us.us.1.i.i = ashr exact i32 %yh.sroa.0.053.us.us.1.i.i, 2
  %xh.sroa.0.0.xm.sroa.0.0.us.us.1.i.i = select i1 %right_major.not.i.i, i64 %xh.sroa.0.052.us.us.1.i.i, i64 %xm.sroa.0.051.us.us.1.i.i
  %xm.sroa.0.0.xh.sroa.0.0.us.us.1.i.i = select i1 %right_major.not.i.i, i64 %xm.sroa.0.051.us.us.1.i.i, i64 %xh.sroa.0.052.us.us.1.i.i
  %_103.us.us.1.i.i = add i64 %xm.sroa.0.0.xh.sroa.0.0.us.us.1.i.i, 262143
  %43 = ashr i64 %_103.us.us.1.i.i, 18
  %spec.store.select.us.us.1.i.i = tail call i64 @llvm.smax.i64(i64 %first_pixel_x.i.i, i64 %43)
  %_108.us.us.1.i.i = add i64 %xh.sroa.0.0.xm.sroa.0.0.us.us.1.i.i, -8
  %_107.us.us.1.i.i = ashr i64 %_108.us.us.1.i.i, 16
  %spec.store.select10.us.us.1.i.i = tail call i64 @llvm.smin.i64(i64 %40, i64 %_107.us.us.1.i.i)
  %px_end.us.us.1.i.i = ashr i64 %spec.store.select10.us.us.1.i.i, 2
  %_114.not45.us.us.1.i.i = icmp sgt i64 %spec.store.select.us.us.1.i.i, %px_end.us.us.1.i.i
  br i1 %_114.not45.us.us.1.i.i, label %bb47.us.us.1.i.i, label %bb38.lr.ph.us.us.1.i.i
bb38.lr.ph.us.us.1.i.i:                           ; preds = %bb36.us.us.1.i.i
  %_120.us.us.1.i.i = mul nuw nsw i32 %_97.us.us.1.i.i, %stride_pixels.i.i
  br label %bb38.us.us.us.1.i.i
bb38.us.us.us.1.i.i:                              ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.1.i.i, %bb38.lr.ph.us.us.1.i.i
  %px_start.sroa.0.046.us.us.us.1.i.i = phi i64 [ %spec.store.select.us.us.1.i.i, %bb38.lr.ph.us.us.1.i.i ], [ %48, %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.1.i.i ]
  %_122.us.us.us.1.i.i = trunc i64 %px_start.sroa.0.046.us.us.us.1.i.i to i32
  %44 = add i32 %_120.us.us.1.i.i, %_122.us.us.us.1.i.i
  %45 = shl i32 %44, 1
  %_9.i.i.us.us.us.1.i.i = add i32 %45, %_117.us.us.us.1.i.i
  %index.i.i.us.us.us.1.i.i = zext i32 %_9.i.i.us.us.us.1.i.i to i64
  %_6.i.i.us.us.us.1.i.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i.i.us.us.us.1.i.i
  br i1 %_6.i.i.us.us.us.1.i.i, label %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.1.i.i, label %panic.i.i.i.i
_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.1.i.i: ; preds = %bb38.us.us.us.1.i.i
  %_9.i2.i.reass.us.us.us.1.i.i = add i32 %_9.i.i.us.us.us.1.i.i, 1
  %index.i3.i.us.us.us.1.i.i = zext i32 %_9.i2.i.reass.us.us.us.1.i.i to i64
  %_6.i5.i.us.us.us.1.i.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i3.i.us.us.us.1.i.i
  br i1 %_6.i5.i.us.us.us.1.i.i, label %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.1.i.i, label %panic.i6.i.i.i
_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.1.i.i: ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.1.i.i
  %48 = add nuw nsw i64 %px_start.sroa.0.046.us.us.us.1.i.i, 1
  %_114.not.us.us.us.not.1.i.i = icmp slt i64 %px_start.sroa.0.046.us.us.us.1.i.i, %px_end.us.us.1.i.i
  br i1 %_114.not.us.us.us.not.1.i.i, label %bb38.us.us.us.1.i.i, label %bb47.us.us.1.i.i
bb47.us.us.1.i.i:                                 ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.1.i.i, %bb36.us.us.1.i.i, %bb32.us.us.1.i.i, %bb30.us.us.1.i.i, %bb29.us.us.1.i.i
  %49 = add i64 %xh.sroa.0.052.us.us.1.i.i, %dh.i.i
  %50 = add i64 %xm.sroa.0.051.us.us.1.i.i, %minor_inc.sroa.0.0.us.1.i.i
  %51 = add i32 %yh.sroa.0.053.us.us.1.i.i, 1
  %exitcond79.1.not.i.i = icmp eq i32 %51, %y_target.sroa.0.0.us.1.i.i
  br i1 %exitcond79.1.not.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb27.us.us.1.i.i
bb27.us.us.i.i:                                   ; preds = %bb47.us.us.i.i, %bb27.us.us.preheader.i.i
  %yh.sroa.0.053.us.us.i.i = phi i32 [ %56, %bb47.us.us.i.i ], [ %22, %bb27.us.us.preheader.i.i ]
  %xh.sroa.0.052.us.us.i.i = phi i64 [ %54, %bb47.us.us.i.i ], [ %27, %bb27.us.us.preheader.i.i ]
  %xm.sroa.0.051.us.us.i.i = phi i64 [ %55, %bb47.us.us.i.i ], [ %30, %bb27.us.us.preheader.i.i ]
  %exitcond78.not.i.i = icmp eq i32 %yh.sroa.0.053.us.us.i.i, %smax77.i.i
  br i1 %exitcond78.not.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb29.us.us.i.i
bb29.us.us.i.i:                                   ; preds = %bb27.us.us.i.i
  %_94.us.us.i.i = and i32 %yh.sroa.0.053.us.us.i.i, 3
  %52 = icmp eq i32 %_94.us.us.i.i, 0
  br i1 %52, label %bb30.us.us.i.i, label %bb47.us.us.i.i
bb30.us.us.i.i:                                   ; preds = %bb29.us.us.i.i
  %_97.us.us.i.i = ashr exact i32 %yh.sroa.0.053.us.us.i.i, 2
  %_96.not.us.us.i.i = icmp slt i32 %_97.us.us.i.i, %first_pixel_y.i.i
  br i1 %_96.not.us.us.i.i, label %bb47.us.us.i.i, label %bb32.us.us.i.i
bb32.us.us.i.i:                                   ; preds = %bb30.us.us.i.i
  %xh.sroa.0.0.xm.sroa.0.0.us.us.i.i = select i1 %right_major.not.i.i, i64 %xh.sroa.0.052.us.us.i.i, i64 %xm.sroa.0.051.us.us.i.i
  %xm.sroa.0.0.xh.sroa.0.0.us.us.i.i = select i1 %right_major.not.i.i, i64 %xm.sroa.0.051.us.us.i.i, i64 %xh.sroa.0.052.us.us.i.i
  %_101.not.us.us.i.i = icmp slt i64 %xh.sroa.0.0.xm.sroa.0.0.us.us.i.i, %xm.sroa.0.0.xh.sroa.0.0.us.us.i.i
  br i1 %_101.not.us.us.i.i, label %bb47.us.us.i.i, label %bb36.us.us.i.i
bb36.us.us.i.i:                                   ; preds = %bb32.us.us.i.i
  %_103.us.us.i.i = add i64 %xm.sroa.0.0.xh.sroa.0.0.us.us.i.i, 262143
  %53 = ashr i64 %_103.us.us.i.i, 18
  %spec.store.select.us.us.i.i = tail call i64 @llvm.smax.i64(i64 %first_pixel_x.i.i, i64 %53)
  %_108.us.us.i.i = add i64 %xh.sroa.0.0.xm.sroa.0.0.us.us.i.i, -8
  %_107.us.us.i.i = ashr i64 %_108.us.us.i.i, 16
  %spec.store.select10.us.us.i.i = tail call i64 @llvm.smin.i64(i64 %40, i64 %_107.us.us.i.i)
  %px_end.us.us.i.i = ashr i64 %spec.store.select10.us.us.i.i, 2
  %_114.not45.us.us.i.i = icmp sgt i64 %spec.store.select.us.us.i.i, %px_end.us.us.i.i
  br i1 %_114.not45.us.us.i.i, label %bb47.us.us.i.i, label %bb38.lr.ph.us.us.i.i
bb47.us.us.i.i:                                   ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.i.i, %bb36.us.us.i.i, %bb32.us.us.i.i, %bb30.us.us.i.i, %bb29.us.us.i.i
  %54 = add i64 %xh.sroa.0.052.us.us.i.i, %dh.i.i
  %55 = add i64 %xm.sroa.0.051.us.us.i.i, %minor_inc.sroa.0.0.us.i.i
  %56 = add i32 %yh.sroa.0.053.us.us.i.i, 1
  %exitcond79.not.i.i = icmp eq i32 %56, %y_target.sroa.0.0.us.i.i
  br i1 %exitcond79.not.i.i, label %bb24.us.i.i, label %bb27.us.us.i.i
bb38.lr.ph.us.us.i.i:                             ; preds = %bb36.us.us.i.i
  %_120.us.us.i.i = mul nuw nsw i32 %_97.us.us.i.i, %stride_pixels.i.i
  br label %bb38.us.us.us.i.i
bb38.us.us.us.i.i:                                ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.i.i, %bb38.lr.ph.us.us.i.i
  %px_start.sroa.0.046.us.us.us.i.i = phi i64 [ %spec.store.select.us.us.i.i, %bb38.lr.ph.us.us.i.i ], [ %61, %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.i.i ]
  %_122.us.us.us.i.i = trunc i64 %px_start.sroa.0.046.us.us.us.i.i to i32
  %57 = add i32 %_120.us.us.i.i, %_122.us.us.us.i.i
  %58 = shl i32 %57, 1
  %_9.i.i.us.us.us.i.i = add i32 %58, %_117.us.us.us.i.i
  %index.i.i.us.us.us.i.i = zext i32 %_9.i.i.us.us.us.i.i to i64
  %_6.i.i.us.us.us.i.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i.i.us.us.us.i.i
  br i1 %_6.i.i.us.us.us.i.i, label %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.i.i, label %panic.i.i.i.i
_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.i.i: ; preds = %bb38.us.us.us.i.i
  %_9.i2.i.reass.us.us.us.i.i = add i32 %_9.i.i.us.us.us.i.i, 1
  %index.i3.i.us.us.us.i.i = zext i32 %_9.i2.i.reass.us.us.us.i.i to i64
  %_6.i5.i.us.us.us.i.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i3.i.us.us.us.i.i
  br i1 %_6.i5.i.us.us.us.i.i, label %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.i.i, label %panic.i6.i.i.i
_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.us.us.us.i.i: ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.i.i
  %61 = add nuw nsw i64 %px_start.sroa.0.046.us.us.us.i.i, 1
  %_114.not.us.us.us.not.i.i = icmp slt i64 %px_start.sroa.0.046.us.us.us.i.i, %px_end.us.us.i.i
  br i1 %_114.not.us.us.us.not.i.i, label %bb38.us.us.us.i.i, label %bb47.us.us.i.i
panic8.i.i:                                       ; preds = %bb16.i.i
  unreachable
bb27.i.i:                                         ; preds = %bb47.i.i, %bb27.preheader.i.i
  %yh.sroa.0.053.i.i = phi i32 [ %73, %bb47.i.i ], [ %22, %bb27.preheader.i.i ]
  %xh.sroa.0.052.i.i = phi i64 [ %71, %bb47.i.i ], [ %27, %bb27.preheader.i.i ]
  %xm.sroa.0.051.i.i = phi i64 [ %72, %bb47.i.i ], [ %30, %bb27.preheader.i.i ]
  %exitcond.not.i.i = icmp eq i32 %yh.sroa.0.053.i.i, %smax.i.i
  br i1 %exitcond.not.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb29.i.i
bb24.i.i:                                         ; preds = %bb47.i.i, %bb20.outer.preheader.i.i
  %xh.sroa.0.0.lcssa.i.i = phi i64 [ %27, %bb20.outer.preheader.i.i ], [ %71, %bb47.i.i ]
  %yh.sroa.0.0.lcssa.i.i = phi i32 [ %22, %bb20.outer.preheader.i.i ], [ %y_target.sroa.0.0.us.i.i, %bb47.i.i ]
  %minor_inc.sroa.0.0.in.1.i.i = shl i64 %_31.i.i, 32
  %minor_inc.sroa.0.0.1.i.i = ashr exact i64 %minor_inc.sroa.0.0.in.1.i.i, 32
  %y_target.sroa.0.0.1.i.i = ashr i32 %tr.sh.diff.i.i, 18
  %_89.not50.1.i.i = icmp slt i32 %yh.sroa.0.0.lcssa.i.i, %y_target.sroa.0.0.1.i.i
  br i1 %_89.not50.1.i.i, label %bb27.preheader.1.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb27.preheader.1.i.i:                             ; preds = %bb24.i.i
  %smax.1.i.i = tail call i32 @llvm.smax.i32(i32 %yh.sroa.0.0.lcssa.i.i, i32 %sc_bottom.i.i)
  br label %bb27.1.i.i
bb27.1.i.i:                                       ; preds = %bb47.1.i.i, %bb27.preheader.1.i.i
  %yh.sroa.0.053.1.i.i = phi i32 [ %69, %bb47.1.i.i ], [ %yh.sroa.0.0.lcssa.i.i, %bb27.preheader.1.i.i ]
  %xh.sroa.0.052.1.i.i = phi i64 [ %67, %bb47.1.i.i ], [ %xh.sroa.0.0.lcssa.i.i, %bb27.preheader.1.i.i ]
  %xm.sroa.0.051.1.i.i = phi i64 [ %68, %bb47.1.i.i ], [ %xl.i.i, %bb27.preheader.1.i.i ]
  %exitcond.1.not.i.i = icmp eq i32 %yh.sroa.0.053.1.i.i, %smax.1.i.i
  br i1 %exitcond.1.not.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb29.1.i.i
bb29.1.i.i:                                       ; preds = %bb27.1.i.i
  %_94.1.i.i = and i32 %yh.sroa.0.053.1.i.i, 3
  %62 = icmp eq i32 %_94.1.i.i, 0
  br i1 %62, label %bb30.1.i.i, label %bb47.1.i.i
bb30.1.i.i:                                       ; preds = %bb29.1.i.i
  %xh.sroa.0.0.xm.sroa.0.0.1.i.i = select i1 %right_major.not.i.i, i64 %xh.sroa.0.052.1.i.i, i64 %xm.sroa.0.051.1.i.i
  %xm.sroa.0.0.xh.sroa.0.0.1.i.i = select i1 %right_major.not.i.i, i64 %xm.sroa.0.051.1.i.i, i64 %xh.sroa.0.052.1.i.i
  %_103.1.i.i = add i64 %xm.sroa.0.0.xh.sroa.0.0.1.i.i, 262143
  %63 = ashr i64 %_103.1.i.i, 18
  %spec.store.select.1.i.i = tail call i64 @llvm.smax.i64(i64 %first_pixel_x.i.i, i64 %63)
  %_108.1.i.i = add i64 %xh.sroa.0.0.xm.sroa.0.0.1.i.i, -8
  %_107.1.i.i = ashr i64 %_108.1.i.i, 16
  %spec.store.select10.1.i.i = tail call i64 @llvm.smin.i64(i64 %40, i64 %_107.1.i.i)
  %px_end.1.i.i = ashr i64 %spec.store.select10.1.i.i, 2
  %_114.not45.1.i.i = icmp sgt i64 %spec.store.select.1.i.i, %px_end.1.i.i
  br i1 %_114.not45.1.i.i, label %bb47.1.i.i, label %bb38.lr.ph.1.i.i
bb38.lr.ph.1.i.i:                                 ; preds = %bb36.1.i.i
  br label %bb38.1.i.i
bb38.1.i.i:                                       ; preds = %bb38.1.i.i, %bb38.lr.ph.1.i.i
  %px_start.sroa.0.046.1.i.i = phi i64 [ %spec.store.select.1.i.i, %bb38.lr.ph.1.i.i ], [ %66, %bb38.1.i.i ]
  %66 = add nuw nsw i64 %px_start.sroa.0.046.1.i.i, 1
  %_114.not.not.1.i.i = icmp slt i64 %px_start.sroa.0.046.1.i.i, %px_end.1.i.i
  br i1 %_114.not.not.1.i.i, label %bb38.1.i.i, label %bb47.1.i.i
bb47.1.i.i:                                       ; preds = %bb38.1.i.i, %bb36.1.i.i, %bb32.1.i.i, %bb30.1.i.i, %bb29.1.i.i
  %67 = add i64 %xh.sroa.0.052.1.i.i, %dh.i.i
  %68 = add i64 %xm.sroa.0.051.1.i.i, %minor_inc.sroa.0.0.1.i.i
  %69 = add i32 %yh.sroa.0.053.1.i.i, 1
  %exitcond75.1.not.i.i = icmp eq i32 %69, %y_target.sroa.0.0.1.i.i
  br i1 %exitcond75.1.not.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb27.1.i.i
bb27.preheader.i.i:                               ; preds = %bb20.outer.preheader.i.i
  %smax.i.i = tail call i32 @llvm.smax.i32(i32 %22, i32 %sc_bottom.i.i)
  br label %bb27.i.i
bb29.i.i:                                         ; preds = %bb27.i.i
  %_94.i.i = and i32 %yh.sroa.0.053.i.i, 3
  %70 = icmp eq i32 %_94.i.i, 0
  br i1 %70, label %bb30.i.i, label %bb47.i.i
bb30.i.i:                                         ; preds = %bb29.i.i
  %xh.sroa.0.0.xm.sroa.0.0.i.i = select i1 %right_major.not.i.i, i64 %xh.sroa.0.052.i.i, i64 %xm.sroa.0.051.i.i
  %xm.sroa.0.0.xh.sroa.0.0.i.i = select i1 %right_major.not.i.i, i64 %xm.sroa.0.051.i.i, i64 %xh.sroa.0.052.i.i
  %_101.not.i.i = icmp slt i64 %xh.sroa.0.0.xm.sroa.0.0.i.i, %xm.sroa.0.0.xh.sroa.0.0.i.i
  br i1 %_101.not.i.i, label %bb47.i.i, label %bb36.i.i
bb47.i.i:                                         ; preds = %bb38.i.i, %bb36.i.i, %bb32.i.i, %bb30.i.i, %bb29.i.i
  %71 = add i64 %xh.sroa.0.052.i.i, %dh.i.i
  %72 = add i64 %xm.sroa.0.051.i.i, %minor_inc.sroa.0.0.us.i.i
  %73 = add i32 %yh.sroa.0.053.i.i, 1
  %exitcond75.not.i.i = icmp eq i32 %73, %y_target.sroa.0.0.us.i.i
  br i1 %exitcond75.not.i.i, label %bb24.i.i, label %bb27.i.i
bb36.i.i:                                         ; preds = %bb32.i.i
  %_103.i.i = add i64 %xm.sroa.0.0.xh.sroa.0.0.i.i, 262143
  %74 = ashr i64 %_103.i.i, 18
  %spec.store.select.i.i = tail call i64 @llvm.smax.i64(i64 %first_pixel_x.i.i, i64 %74)
  %_108.i.i = add i64 %xh.sroa.0.0.xm.sroa.0.0.i.i, -8
  %_107.i.i = ashr i64 %_108.i.i, 16
  %spec.store.select10.i.i = tail call i64 @llvm.smin.i64(i64 %40, i64 %_107.i.i)
  %px_end.i.i = ashr i64 %spec.store.select10.i.i, 2
  %_114.not45.i.i = icmp sgt i64 %spec.store.select.i.i, %px_end.i.i
  br i1 %_114.not45.i.i, label %bb47.i.i, label %bb38.lr.ph.i.i
bb38.lr.ph.i.i:                                   ; preds = %bb36.i.i
  br label %bb38.i.i
bb38.i.i:                                         ; preds = %bb38.i.i, %bb38.lr.ph.i.i
  %px_start.sroa.0.046.i.i = phi i64 [ %spec.store.select.i.i, %bb38.lr.ph.i.i ], [ %77, %bb38.i.i ]
  %77 = add nuw nsw i64 %px_start.sroa.0.046.i.i, 1
  %_114.not.not.i.i = icmp slt i64 %px_start.sroa.0.046.i.i, %px_end.i.i
  br i1 %_114.not.not.i.i, label %bb38.i.i, label %bb47.i.i
panic.i.i.i.i:                                    ; preds = %bb38.us.us.us.i.i, %bb38.us.us.us.1.i.i
  unreachable
panic.i6.i.i.i:                                   ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.i.i, %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.us.us.us.1.i.i
  unreachable
bb12.i:                                           ; preds = %bb6.i.i, %bb33.i
  %78 = add i32 %7, 1
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb34.i:                                           ; preds = %bb4.i
  %79 = add i32 %7, 1
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb16.i:                                           ; preds = %bb3.i
  %80 = and i64 %_107779, 3435922176
  %or.cond81.i = icmp eq i64 %80, 2152464896
  br i1 %or.cond81.i, label %bb6.i16, label %bb21.i
bb6.i16:                                          ; preds = %bb16.i
  %_10.i = lshr i64 %_10, 44
  %81 = trunc nuw nsw i64 %_10.i to i32
  %xl.i17 = and i32 %81, 4095
  %_13.i18 = lshr i64 %_10, 32
  %82 = trunc nuw i64 %_13.i18 to i32
  %yl.i19 = and i32 %82, 4095
  %83 = trunc i64 %_10 to i32
  %84 = lshr i32 %83, 12
  %85 = and i32 %84, 4095
  %86 = and i32 %83, 4095
  %_122.i = lshr i64 %_37.i82, 44
  %87 = trunc nuw nsw i64 %_122.i to i32
  %sc_left.i = and i32 %87, 4095
  %_126.i = lshr i64 %_37.i82, 32
  %88 = trunc nuw i64 %_126.i to i32
  %sc_top.i = and i32 %88, 4095
  %89 = trunc i64 %_37.i82 to i32
  %90 = lshr i32 %89, 12
  %sc_right.i20 = and i32 %90, 4095
  %sc_bottom.i21 = and i32 %89, 4095
  %spec.store.select.i22 = tail call i32 @llvm.umax.i32(i32 %sc_left.i, i32 %85)
  %spec.store.select9.i = tail call i32 @llvm.umin.i32(i32 %sc_right.i20, i32 %xl.i17)
  %spec.store.select8.i = tail call i32 @llvm.umax.i32(i32 %sc_top.i, i32 %86)
  %spec.store.select10.i = tail call i32 @llvm.umin.i32(i32 %sc_bottom.i21, i32 %yl.i19)
  %_23.not.i = icmp samesign ugt i32 %spec.store.select9.i, %spec.store.select.i22
  %_26.not.i = icmp samesign ugt i32 %spec.store.select10.i, %spec.store.select8.i
  %or.cond23.i = select i1 %_23.not.i, i1 %_26.not.i, i1 false
  br i1 %or.cond23.i, label %bb11.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb11.i:                                           ; preds = %bb6.i16
  %_30.i = add nuw nsw i32 %spec.store.select.i22, 3
  %x0.i = lshr i32 %_30.i, 2
  %_32.i = add nsw i32 %spec.store.select9.i, -1
  %x1.i23 = lshr i32 %_32.i, 2
  %_34.i = add nuw nsw i32 %spec.store.select8.i, 3
  %y0.i = lshr i32 %_34.i, 2
  %_36.i = add nsw i32 %spec.store.select10.i, -1
  %y1.i24 = lshr i32 %_36.i, 2
  %_37.i25 = icmp samesign ult i32 %x1.i23, %x0.i
  %_38.i = icmp samesign ult i32 %y1.i24, %y0.i
  %or.cond.i = or i1 %_37.i25, %_38.i
  br i1 %or.cond.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb13.i
bb13.i:                                           ; preds = %bb11.i
  %91 = trunc i64 %state.val.i85 to i32
  %base.i = and i32 %91, 67108863
  %_139.i = lshr i64 %state.val.i85, 32
  %92 = trunc nuw i64 %_139.i to i32
  %_47.i = and i32 %92, 1023
  %stride_pixels.i = add nuw nsw i32 %_47.i, 1
  %_141.i = lshr i64 %state.val.i85, 51
  %93 = trunc nuw nsw i64 %_141.i to i32
  %_48.i = and i32 %93, 3
  switch i32 %_48.i, label %bb21.i [
  ]
bb21.preheader.i:                                 ; preds = %bb13.i
  %97 = and i64 %_122.i, 4095
  %98 = zext nneg i32 %85 to i64
  %umax.i27 = tail call i64 @llvm.umax.i64(i64 %97, i64 %98)
  %99 = add nuw nsw i64 %umax.i27, 3
  %100 = lshr i64 %99, 2
  %_76.i = sub i32 %base.i, %_10.i.i.i14.i
  %narrow = add nuw nsw i32 %x1.i23, 1
  %101 = zext nneg i32 %narrow to i64
  br label %bb21.i28
bb30.preheader.i:                                 ; preds = %bb13.i
  br label %bb30.i
bb23.bb18.loopexit_crit_edge.i:                   ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i
  %exitcond130.not = icmp eq i32 %iter.sroa.0.098.i, %y1.i24
  br i1 %exitcond130.not, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb21.i28
bb21.i28:                                         ; preds = %bb23.bb18.loopexit_crit_edge.i, %bb21.preheader.i
  %iter.sroa.0.098.i = phi i32 [ %102, %bb23.bb18.loopexit_crit_edge.i ], [ %y0.i, %bb21.preheader.i ]
  %102 = add nuw nsw i32 %iter.sroa.0.098.i, 1
  %_67.i = mul nuw nsw i32 %iter.sroa.0.098.i, %stride_pixels.i
  br label %bb25.i
bb25.i:                                           ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i, %bb21.i28
  %indvars.iv.i29 = phi i64 [ %100, %bb21.i28 ], [ %indvars.iv.next.i30, %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i ]
  %indvars.iv.next.i30 = add nuw nsw i64 %indvars.iv.i29, 1
  %103 = trunc nuw nsw i64 %indvars.iv.i29 to i32
  %104 = add nuw nsw i32 %_67.i, %103
  %105 = shl nuw nsw i32 %104, 1
  %_9.i.i.i31 = add i32 %_76.i, %105
  %index.i.i.i = zext i32 %_9.i.i.i31 to i64
  %_6.i.i.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i.i.i
  br i1 %_6.i.i.i, label %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i, label %panic.i.i.i
panic.i.i.i:                                      ; preds = %bb25.i
  unreachable
_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i: ; preds = %bb25.i
  unreachable
_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i: ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i
  %exitcond129.not = icmp eq i64 %indvars.iv.next.i30, %101
  br i1 %exitcond129.not, label %bb23.bb18.loopexit_crit_edge.i, label %bb25.i
bb32.bb28.loopexit_crit_edge.i:                   ; preds = %bb34.i26
  %108 = add nuw nsw i32 %iter2.sroa.0.094.i, 1
  %exitcond128.not = icmp eq i32 %iter2.sroa.0.094.i, %y1.i24
  br i1 %exitcond128.not, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb30.i
bb30.i:                                           ; preds = %bb32.bb28.loopexit_crit_edge.i, %bb30.preheader.i
  %iter2.sroa.0.094.i = phi i32 [ %108, %bb32.bb28.loopexit_crit_edge.i ], [ %y0.i, %bb30.preheader.i ]
  br label %bb34.i26
bb34.i26:                                         ; preds = %bb34.i26, %bb30.i
  %iter3.sroa.0.092.i = phi i32 [ %x0.i, %bb30.i ], [ %109, %bb34.i26 ]
  %109 = add nuw nsw i32 %iter3.sroa.0.092.i, 1
  %exitcond127.not = icmp eq i32 %iter3.sroa.0.092.i, %x1.i23
  br i1 %exitcond127.not, label %bb32.bb28.loopexit_crit_edge.i, label %bb34.i26
bb15.i:                                           ; preds = %bb3.i, %bb3.i
  %112 = add i32 %7, 1
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
bb17.i:                                           ; preds = %bb3.i
  %_6.i = lshr i64 %_10, 44
  %113 = trunc nuw nsw i64 %_6.i to i32
  %xl.i = and i32 %113, 4095
  %_9.i = lshr i64 %_10, 32
  %114 = trunc nuw i64 %_9.i to i32
  %yl.i = and i32 %114, 4095
  %115 = trunc i64 %_10 to i32
  %116 = lshr i32 %115, 14
  %xh.i = and i32 %116, 1023
  %117 = trunc i64 %_37.i82 to i32
  %118 = lshr i32 %117, 12
  %sc_right.i = and i32 %118, 4095
  %sc_bottom.i = and i32 %117, 4095
  %sum.shift.i = lshr i64 %_37.i82, 46
  %sc_left9.i = trunc nuw nsw i64 %sum.shift.i to i32
  %_17.i8 = and i32 %sc_left9.i, 1023
  %spec.store.select.i = tail call i32 @llvm.umax.i32(i32 %_17.i8, i32 %xh.i)
  %spec.store.select2.i = tail call i32 @llvm.umin.i32(i32 %sc_right.i, i32 %xl.i)
  %x1.i = lshr i32 %spec.store.select2.i, 2
  %yh.i = lshr i32 %115, 2
  %119 = and i32 %yh.i, 1023
  %sum.shift10.i = lshr i64 %_37.i82, 34
  %sc_top11.i = trunc nuw nsw i64 %sum.shift10.i to i32
  %_22.i9 = and i32 %sc_top11.i, 1023
  %spec.store.select1.i = tail call i32 @llvm.umax.i32(i32 %_22.i9, i32 %119)
  %120 = add nsw i32 %sc_bottom.i, -1
  %spec.store.select3.i = tail call i32 @llvm.smin.i32(i32 %120, i32 %yl.i)
  %y1.i = ashr i32 %spec.store.select3.i, 2
  %_0.i.not.i53.i = icmp sgt i32 %spec.store.select1.i, %y1.i
  br i1 %_0.i.not.i53.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb4.lr.ph.i
bb4.lr.ph.i:                                      ; preds = %bb17.i
  %121 = trunc i64 %state.val.i85 to i32
  %base.i.i10 = and i32 %121, 67108863
  %_45.i.i = lshr i64 %state.val.i85, 32
  %122 = trunc nuw i64 %_45.i.i to i32
  %_8.i.i = and i32 %122, 1023
  %stride_pixels.i.i11 = add nuw nsw i32 %_8.i.i, 1
  %_47.i.i = lshr i64 %state.val.i85, 51
  %123 = trunc nuw nsw i64 %_47.i.i to i32
  %_10.i.i = and i32 %123, 3
  %124 = zext nneg i32 %spec.store.select.i to i64
  %_24.i.i12 = sub i32 %base.i.i10, %_10.i.i.i14.i
  switch i32 %_10.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit [
  ]
bb4.lr.ph.split.us.i:                             ; preds = %bb4.lr.ph.i
  %_0.i.not.i16.i.i = icmp samesign ugt i32 %spec.store.select.i, %x1.i
  br i1 %_0.i.not.i16.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb4.us.preheader.i
bb4.us.preheader.i:                               ; preds = %bb4.lr.ph.split.us.i
  %narrow.i = add nuw nsw i32 %x1.i, 1
  %125 = zext nneg i32 %narrow.i to i64
  %126 = tail call i32 @llvm.umax.i32(i32 %y1.i, i32 %spec.store.select1.i)
  br label %bb4.us.i
bb4.us.i:                                         ; preds = %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i, %bb4.us.preheader.i
  %iter.sroa.0.054.us.i = phi i32 [ %127, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i ], [ %spec.store.select1.i, %bb4.us.preheader.i ]
  %127 = add nuw nsw i32 %iter.sroa.0.054.us.i, 1
  %_13.i.us.i = mul nuw nsw i32 %iter.sroa.0.054.us.i, %stride_pixels.i.i11
  br label %bb6.i.us.i
bb6.i.us.i:                                       ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i.us.i, %bb4.us.i
  %indvars.iv.i.us.i = phi i64 [ %124, %bb4.us.i ], [ %indvars.iv.next.i.us.i, %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i.us.i ]
  %indvars.iv.next.i.us.i = add nuw nsw i64 %indvars.iv.i.us.i, 1
  %indvars.iv.tr.i.us.i = trunc i64 %indvars.iv.i.us.i to i32
  %131 = add i32 %_13.i.us.i, %indvars.iv.tr.i.us.i
  %132 = shl i32 %131, 1
  %_9.i.i.i.us.i = add i32 %132, %_24.i.i12
  %index.i.i.i.us.i = zext i32 %_9.i.i.i.us.i to i64
  %_6.i.i.i.us.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i.i.i.us.i
  br i1 %_6.i.i.i.us.i, label %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i.us.i, label %panic.i.i.i.i14
_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i.us.i: ; preds = %bb6.i.us.i
  %_9.i2.i.reass.i.us.i = add i32 %_9.i.i.i.us.i, 1
  %index.i3.i.i.us.i = zext i32 %_9.i2.i.reass.i.us.i to i64
  %_6.i5.i.i.us.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i3.i.i.us.i
  br i1 %_6.i5.i.i.us.i, label %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i.us.i, label %panic.i6.i.i.i15
_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i.us.i: ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i.us.i
  %exitcond93.not.i = icmp eq i64 %indvars.iv.next.i.us.i, %125
  br i1 %exitcond93.not.i, label %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i, label %bb6.i.us.i
_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i: ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i.us.i
  %exitcond126.not = icmp eq i32 %iter.sroa.0.054.us.i, %126
  br i1 %exitcond126.not, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb4.us.i
bb4.lr.ph.split.us58.i:                           ; preds = %bb4.lr.ph.i
  %_0.i.not.i1214.i.i = icmp samesign ugt i32 %spec.store.select.i, %x1.i
  br i1 %_0.i.not.i1214.i.i, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb4.us59.preheader.i
bb4.us59.preheader.i:                             ; preds = %bb4.lr.ph.split.us58.i
  %135 = and i64 %sum.shift.i, 1023
  %136 = zext nneg i32 %xh.i to i64
  %umax.i = tail call i64 @llvm.umax.i64(i64 %135, i64 %136)
  %137 = trunc nuw nsw i64 %umax.i to i32
  %138 = add nuw nsw i32 %137, 1
  %139 = sub nsw i32 %138, %spec.store.select.i
  %140 = add nsw i32 %139, %x1.i
  %wide.trip.count.i = zext i32 %140 to i64
  %141 = tail call i32 @llvm.umax.i32(i32 %y1.i, i32 %spec.store.select1.i)
  br label %bb4.us59.i
bb4.us59.i:                                       ; preds = %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i, %bb4.us59.preheader.i
  %iter.sroa.0.054.us60.i = phi i32 [ %142, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i ], [ %spec.store.select1.i, %bb4.us59.preheader.i ]
  %142 = add nuw nsw i32 %iter.sroa.0.054.us60.i, 1
  %_30.i.us.i = mul nuw nsw i32 %iter.sroa.0.054.us60.i, %stride_pixels.i.i11
  br label %bb14.i.us.i
bb14.i.us.i:                                      ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u32Cs5ertoHtPCbu_14n64_systemtest.exit.us.i, %bb4.us59.i
  %indvars.iv.i = phi i64 [ %umax.i, %bb4.us59.i ], [ %indvars.iv.next.i, %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u32Cs5ertoHtPCbu_14n64_systemtest.exit.us.i ]
  %indvars.iv.next.i = add nuw nsw i64 %indvars.iv.i, 1
  %143 = trunc nuw nsw i64 %indvars.iv.i to i32
  %144 = add i32 %_30.i.us.i, %143
  %145 = shl i32 %144, 2
  %_9.i.i.i15.us.i = add i32 %_24.i.i12, %145
  %index.i.i.i16.us.i = zext i32 %_9.i.i.i15.us.i to i64
  %_6.i.i.i18.us.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i.i.i16.us.i
  br i1 %_6.i.i.i18.us.i, label %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i20.us.i, label %panic.i.i.i19.i
_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i20.us.i: ; preds = %bb14.i.us.i
  %_9.i2.i.i.reass.us.i = add i32 %_9.i.i.i15.us.i, 1
  %index.i3.i.i23.us.i = zext i32 %_9.i2.i.i.reass.us.i to i64
  %_6.i5.i.i24.us.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i3.i.i23.us.i
  br i1 %_6.i5.i.i24.us.i, label %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i26.us.i, label %panic.i6.i.i25.i
_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u16Cs5ertoHtPCbu_14n64_systemtest.exit.i26.us.i: ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i20.us.i
  %_9.i2.i12.i.reass.us.i = add i32 %_9.i.i.i15.us.i, 3
  %index.i3.i13.i.us.i = zext i32 %_9.i2.i12.i.reass.us.i to i64
  %_6.i5.i14.i.us.i = icmp ugt i64 %_7.1.i.i.i17.i, %index.i3.i13.i.us.i
  br i1 %_6.i5.i14.i.us.i, label %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u32Cs5ertoHtPCbu_14n64_systemtest.exit.us.i, label %panic.i6.i15.i.i
_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u32Cs5ertoHtPCbu_14n64_systemtest.exit.us.i: ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i7.i.us.i
  %exitcond.not.i = icmp eq i64 %indvars.iv.next.i, %wide.trip.count.i
  br i1 %exitcond.not.i, label %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i, label %bb14.i.us.i
_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i: ; preds = %_RNvYNtNtCshhILhMXdhv1_8rdp_core5rdram10SliceRdramNtB4_5Rdram9write_u32Cs5ertoHtPCbu_14n64_systemtest.exit.us.i
  %exitcond.not = icmp eq i32 %iter.sroa.0.054.us60.i, %141
  br i1 %exitcond.not, label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit, label %bb4.us59.i
panic.i.i.i.i14:                                  ; preds = %bb6.i.us.i
  unreachable
panic.i6.i.i.i15:                                 ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i.us.i
  unreachable
panic.i.i.i19.i:                                  ; preds = %bb14.i.us.i
  unreachable
panic.i6.i.i25.i:                                 ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i.i20.us.i
  unreachable
panic.i6.i15.i.i:                                 ; preds = %_RNvXs_NtCshhILhMXdhv1_8rdp_core5rdramNtB4_10SliceRdramNtB4_5Rdram8write_u8.exit.i7.i.us.i
  unreachable
bb21.i:                                           ; preds = %bb13.i, %bb16.i
  %150 = add i32 %7, 1
  br label %_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit
_RINvMNtCshhILhMXdhv1_8rdp_core4softNtB3_7SoftRdp7executeNtNtB5_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit: ; preds = %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i, %bb32.bb28.loopexit_crit_edge.i, %bb23.bb18.loopexit_crit_edge.i, %bb27.i.i, %bb27.1.i.i, %bb47.1.i.i, %bb27.us.us.i.i, %bb27.us.us.1.i.i, %bb47.us.us.1.i.i, %bb11.i, %bb6.i16, %bb4.lr.ph.split.us58.i, %bb4.lr.ph.split.us.i, %bb4.lr.ph.i, %bb17.i, %bb20, %bb20, %bb20, %bb20, %bb20, %bb2.i, %bb9.i, %bb8.i, %bb7.i, %bb6.i, %bb5.i, %bb24.us.i.i, %bb24.i.i, %bb12.i, %bb34.i, %bb15.i, %bb21.i
  %blend.i91 = phi i32 [ %blend.i92, %bb27.i.i ], [ %blend.i92, %bb23.bb18.loopexit_crit_edge.i ], [ %blend.i92, %bb27.us.us.i.i ], [ %blend.i92, %bb27.1.i.i ], [ %blend.i92, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i ], [ %blend.i92, %bb32.bb28.loopexit_crit_edge.i ], [ %blend.i92, %bb27.us.us.1.i.i ], [ %blend.i92, %bb21.i ], [ %blend.i92, %bb11.i ], [ %blend.i92, %bb6.i16 ], [ %blend.i92, %bb4.lr.ph.split.us58.i ], [ %blend.i92, %bb4.lr.ph.split.us.i ], [ %blend.i92, %bb4.lr.ph.i ], [ %blend.i92, %bb17.i ], [ %blend.i92, %bb20 ], [ %blend.i92, %bb20 ], [ %blend.i92, %bb20 ], [ %blend.i92, %bb20 ], [ %blend.i92, %bb20 ], [ %blend.i92, %bb2.i ], [ %blend.i92, %bb9.i ], [ %blend.i92, %bb8.i ], [ %blend.i92, %bb7.i ], [ %13, %bb6.i ], [ %blend.i92, %bb5.i ], [ %blend.i92, %bb24.us.i.i ], [ %blend.i92, %bb24.i.i ], [ %blend.i92, %bb12.i ], [ %blend.i92, %bb34.i ], [ %blend.i92, %bb15.i ], [ %blend.i92, %bb47.us.us.1.i.i ], [ %blend.i92, %bb47.1.i.i ], [ %blend.i92, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i ]
  %151 = phi i32 [ %7, %bb27.i.i ], [ %7, %bb23.bb18.loopexit_crit_edge.i ], [ %7, %bb27.us.us.i.i ], [ %7, %bb27.1.i.i ], [ %7, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i ], [ %7, %bb32.bb28.loopexit_crit_edge.i ], [ %7, %bb27.us.us.1.i.i ], [ %150, %bb21.i ], [ %7, %bb11.i ], [ %7, %bb6.i16 ], [ %7, %bb4.lr.ph.split.us58.i ], [ %7, %bb4.lr.ph.split.us.i ], [ %7, %bb4.lr.ph.i ], [ %7, %bb17.i ], [ %7, %bb20 ], [ %7, %bb20 ], [ %7, %bb20 ], [ %7, %bb20 ], [ %7, %bb20 ], [ %11, %bb2.i ], [ %7, %bb9.i ], [ %7, %bb8.i ], [ %7, %bb7.i ], [ %7, %bb6.i ], [ %7, %bb5.i ], [ %7, %bb24.us.i.i ], [ %7, %bb24.i.i ], [ %78, %bb12.i ], [ %79, %bb34.i ], [ %112, %bb15.i ], [ %7, %bb47.us.us.1.i.i ], [ %7, %bb47.1.i.i ], [ %7, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i ]
  %state.val12.i87 = phi i32 [ %state.val12.i88, %bb27.i.i ], [ %state.val12.i88, %bb23.bb18.loopexit_crit_edge.i ], [ %state.val12.i88, %bb27.us.us.i.i ], [ %state.val12.i88, %bb27.1.i.i ], [ %state.val12.i88, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i ], [ %state.val12.i88, %bb32.bb28.loopexit_crit_edge.i ], [ %state.val12.i88, %bb27.us.us.1.i.i ], [ %state.val12.i88, %bb21.i ], [ %state.val12.i88, %bb11.i ], [ %state.val12.i88, %bb6.i16 ], [ %state.val12.i88, %bb4.lr.ph.split.us58.i ], [ %state.val12.i88, %bb4.lr.ph.split.us.i ], [ %state.val12.i88, %bb4.lr.ph.i ], [ %state.val12.i88, %bb17.i ], [ %state.val12.i88, %bb20 ], [ %state.val12.i88, %bb20 ], [ %state.val12.i88, %bb20 ], [ %state.val12.i88, %bb20 ], [ %state.val12.i88, %bb20 ], [ %state.val12.i88, %bb2.i ], [ %state.val12.i88, %bb9.i ], [ %state.val12.i88, %bb8.i ], [ %12, %bb7.i ], [ %state.val12.i88, %bb6.i ], [ %state.val12.i88, %bb5.i ], [ %state.val12.i88, %bb24.us.i.i ], [ %state.val12.i88, %bb24.i.i ], [ %state.val12.i88, %bb12.i ], [ %state.val12.i88, %bb34.i ], [ %state.val12.i88, %bb15.i ], [ %state.val12.i88, %bb47.us.us.1.i.i ], [ %state.val12.i88, %bb47.1.i.i ], [ %state.val12.i88, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i ]
  %state.val.i84 = phi i64 [ %state.val.i85, %bb27.i.i ], [ %state.val.i85, %bb23.bb18.loopexit_crit_edge.i ], [ %state.val.i85, %bb27.us.us.i.i ], [ %state.val.i85, %bb27.1.i.i ], [ %state.val.i85, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i ], [ %state.val.i85, %bb32.bb28.loopexit_crit_edge.i ], [ %state.val.i85, %bb27.us.us.1.i.i ], [ %state.val.i85, %bb21.i ], [ %state.val.i85, %bb11.i ], [ %state.val.i85, %bb6.i16 ], [ %state.val.i85, %bb4.lr.ph.split.us58.i ], [ %state.val.i85, %bb4.lr.ph.split.us.i ], [ %state.val.i85, %bb4.lr.ph.i ], [ %state.val.i85, %bb17.i ], [ %state.val.i85, %bb20 ], [ %state.val.i85, %bb20 ], [ %state.val.i85, %bb20 ], [ %state.val.i85, %bb20 ], [ %state.val.i85, %bb20 ], [ %state.val.i85, %bb2.i ], [ %state.val.i85, %bb9.i ], [ %state.val.i85, %bb8.i ], [ %state.val.i85, %bb7.i ], [ %state.val.i85, %bb6.i ], [ %_10, %bb5.i ], [ %state.val.i85, %bb24.us.i.i ], [ %state.val.i85, %bb24.i.i ], [ %state.val.i85, %bb12.i ], [ %state.val.i85, %bb34.i ], [ %state.val.i85, %bb15.i ], [ %state.val.i85, %bb47.us.us.1.i.i ], [ %state.val.i85, %bb47.1.i.i ], [ %state.val.i85, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i ]
  %_37.i81 = phi i64 [ %_37.i82, %bb27.i.i ], [ %_37.i82, %bb23.bb18.loopexit_crit_edge.i ], [ %_37.i82, %bb27.us.us.i.i ], [ %_37.i82, %bb27.1.i.i ], [ %_37.i82, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i ], [ %_37.i82, %bb32.bb28.loopexit_crit_edge.i ], [ %_37.i82, %bb27.us.us.1.i.i ], [ %_37.i82, %bb21.i ], [ %_37.i82, %bb11.i ], [ %_37.i82, %bb6.i16 ], [ %_37.i82, %bb4.lr.ph.split.us58.i ], [ %_37.i82, %bb4.lr.ph.split.us.i ], [ %_37.i82, %bb4.lr.ph.i ], [ %_37.i82, %bb17.i ], [ %_37.i82, %bb20 ], [ %_37.i82, %bb20 ], [ %_37.i82, %bb20 ], [ %_37.i82, %bb20 ], [ %_37.i82, %bb20 ], [ %_37.i82, %bb2.i ], [ %_10, %bb9.i ], [ %_37.i82, %bb8.i ], [ %_37.i82, %bb7.i ], [ %_37.i82, %bb6.i ], [ %_37.i82, %bb5.i ], [ %_37.i82, %bb24.us.i.i ], [ %_37.i82, %bb24.i.i ], [ %_37.i82, %bb12.i ], [ %_37.i82, %bb34.i ], [ %_37.i82, %bb15.i ], [ %_37.i82, %bb47.us.us.1.i.i ], [ %_37.i82, %bb47.1.i.i ], [ %_37.i82, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i ]
  %_1076 = phi i64 [ %_107779, %bb27.i.i ], [ %_107779, %bb23.bb18.loopexit_crit_edge.i ], [ %_107779, %bb27.us.us.i.i ], [ %_107779, %bb27.1.i.i ], [ %_107779, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit.us.i ], [ %_107779, %bb32.bb28.loopexit_crit_edge.i ], [ %_107779, %bb27.us.us.1.i.i ], [ %_107779, %bb21.i ], [ %_107779, %bb11.i ], [ %_107779, %bb6.i16 ], [ %_107779, %bb4.lr.ph.split.us58.i ], [ %_107779, %bb4.lr.ph.split.us.i ], [ %_107779, %bb4.lr.ph.i ], [ %_107779, %bb17.i ], [ %_107779, %bb20 ], [ %_107779, %bb20 ], [ %_107779, %bb20 ], [ %_107779, %bb20 ], [ %_107779, %bb20 ], [ %_107779, %bb2.i ], [ %_107779, %bb9.i ], [ %_10, %bb8.i ], [ %_107779, %bb7.i ], [ %_107779, %bb6.i ], [ %_107779, %bb5.i ], [ %_107779, %bb24.us.i.i ], [ %_107779, %bb24.i.i ], [ %_107779, %bb12.i ], [ %_107779, %bb34.i ], [ %_107779, %bb15.i ], [ %_107779, %bb47.us.us.1.i.i ], [ %_107779, %bb47.1.i.i ], [ %_107779, %_RINvNtCshhILhMXdhv1_8rdp_core6raster9fill_spanNtNtB4_5rdram10SliceRdramECs5ertoHtPCbu_14n64_systemtest.exit.loopexit32.us.i ]
  %_5 = icmp samesign ult i64 %_14, %stream.1
  br i1 %_5, label %bb3, label %bb8
}