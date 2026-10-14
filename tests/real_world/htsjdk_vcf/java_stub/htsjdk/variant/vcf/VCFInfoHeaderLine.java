package htsjdk.variant.vcf;

public class VCFInfoHeaderLine {
    public VCFHeaderLineCount getCountType() { return VCFHeaderLineCount.UNBOUNDED; }
    public int getCount() { return -1; }
}
