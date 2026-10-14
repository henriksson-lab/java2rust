package htsjdk.variant.variantcontext;

import htsjdk.variant.vcf.VCFHeader;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Set;

public class VariantContext {
    public String getContig() { return ""; }
    public int getStart() { return 0; }
    public String getID() { return "."; }
    public Allele getReference() { return Allele.NO_CALL; }
    public boolean isVariant() { return false; }
    public Allele getAlternateAllele(final int index) { return Allele.NO_CALL; }
    public List<Allele> getAlternateAlleles() { return Collections.emptyList(); }
    public boolean hasLog10PError() { return false; }
    public double getPhredScaledQual() { return 0.0; }
    public boolean isFiltered() { return false; }
    public Set<String> getFilters() { return Collections.emptySet(); }
    public boolean filtersWereApplied() { return false; }
    public Map<String, Object> getAttributes() { return Collections.emptyMap(); }
    public GenotypesContext getGenotypes() { return new GenotypesContext(); }
    public List<String> calcVCFGenotypeKeys(final VCFHeader header) { return Collections.emptyList(); }
    public int getMaxPloidy(final int defaultPloidy) { return defaultPloidy; }
    public Genotype getGenotype(final String sample) { return null; }
    public List<Allele> getAlleles() { return Collections.emptyList(); }
}
