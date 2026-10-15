package org.biojava.nbio.core.sequence.template;

import java.util.List;

public interface Sequence<C extends Compound> extends Iterable<C> {
    int getLength();
    C getCompoundAt(int position);
    int getIndexOf(C compound);
    int getLastIndexOf(C compound);
    String getSequenceAsString();
    List<C> getAsList();
    Sequence<C> getSubSequence(Integer start, Integer end);
    CompoundSet<C> getCompoundSet();
    int countCompounds(C... compounds);
    Sequence<C> getInverse();
}

