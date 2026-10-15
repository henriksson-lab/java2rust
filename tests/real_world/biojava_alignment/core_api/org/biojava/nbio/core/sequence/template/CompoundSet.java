package org.biojava.nbio.core.sequence.template;

import java.util.List;
import java.util.Set;

public interface CompoundSet<C extends Compound> {
    int getMaxSingleCompoundStringLength();
    boolean isCompoundStringLengthEqual();
    C getCompoundForString(String compound);
    String getStringForCompound(C compound);
    boolean compoundsEquivalent(C first, C second);
    Set<C> getEquivalentCompounds(C compound);
    boolean hasCompound(C compound);
    List<C> getAllCompounds();
    boolean isValidSequence(Sequence<C> sequence);
}

