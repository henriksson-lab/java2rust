package htsjdk.variant.variantcontext;

public class Allele {
    public static final Allele NO_CALL = new Allele();
    public String getDisplayString() { return "."; }
}
