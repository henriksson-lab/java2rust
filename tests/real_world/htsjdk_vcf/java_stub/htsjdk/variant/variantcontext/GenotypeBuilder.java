package htsjdk.variant.variantcontext;

public final class GenotypeBuilder {
    public static Genotype createMissing(final String sample, final int ploidy) { return new Genotype(); }
}
