package org.biojava.nbio.core.alignment.template;

/**
 * Summary-only declaration for the BioJava Core dependency used by the
 * Alignment corpus. The real interface also extends Profile; only members
 * directly consumed by the untouched corpus are declared here.
 */
public interface SequencePair<S, C> {
    AlignedSequence<S, C> getQuery();
    AlignedSequence<S, C> getTarget();
    int getLength();
    int getNumIdenticals();
    int getNumSimilars();
    C getCompoundAt(int sequence, int alignmentIndex);
}
