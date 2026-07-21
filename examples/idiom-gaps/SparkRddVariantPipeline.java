package idiom.gaps;

import java.util.Comparator;

class SparkRddVariantPipeline {
    JavaRDD<VariantContext> sort(final JavaRDD<VariantContext> variants, final VCFHeader header, final int reducers) {
        final JavaPairRDD<VariantContext, Void> pairs = variants.mapToPair(variant -> new Tuple2<>(variant, (Void) null));
        final Comparator<VariantContext> comparator = header.getVCFRecordComparator();
        final JavaPairRDD<VariantContext, Void> sorted;
        if (comparator == null) {
            sorted = pairs;
        } else if (reducers > 0) {
            sorted = pairs.sortByKey(comparator, true, reducers);
        } else {
            sorted = pairs.sortByKey(comparator);
        }
        return sorted.map(Tuple2::_1);
    }
}
