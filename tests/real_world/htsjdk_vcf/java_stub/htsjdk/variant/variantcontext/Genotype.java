package htsjdk.variant.variantcontext;

public class Genotype {
    public Allele getAllele(final int index) { return Allele.NO_CALL; }
    public int getPloidy() { return 0; }
    public boolean isPhased() { return false; }
    public boolean isAvailable() { return false; }
    public boolean isFiltered() { return false; }
    public String getFilters() { return ""; }
    public boolean hasExtendedAttribute(final String key) { return false; }
    public Object getExtendedAttribute(final String key) { return null; }
}
