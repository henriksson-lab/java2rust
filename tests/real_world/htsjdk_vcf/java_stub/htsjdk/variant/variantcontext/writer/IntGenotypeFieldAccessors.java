package htsjdk.variant.variantcontext.writer;

import htsjdk.variant.variantcontext.Genotype;

public class IntGenotypeFieldAccessors {
    public Accessor getAccessor(final String field) { return null; }

    public static class Accessor {
        public int[] getValues(final Genotype genotype) { return null; }
    }
}
