package idiom.gaps;

class DisqExternalBoundaryChain {
    JavaRDD<VariantContext> read(final JavaSparkContext ctx, final String input, final java.util.List<SimpleInterval> intervals) {
        return HtsjdkVariantsRddStorage.makeDefault(ctx)
                .read(input, intervals)
                .getVariants();
    }

    void write(final JavaSparkContext ctx, final VCFHeader header, final JavaRDD<VariantContext> variants, final String output) {
        HtsjdkVariantsRdd rdd = new HtsjdkVariantsRdd(header, variants);
        HtsjdkVariantsRddStorage.makeDefault(ctx).write(rdd, output, TabixIndexWriteOption.fromBoolean(true));
    }
}
