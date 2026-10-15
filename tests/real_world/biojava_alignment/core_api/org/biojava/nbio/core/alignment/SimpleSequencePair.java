package org.biojava.nbio.core.alignment;

import java.util.List;
import org.biojava.nbio.core.alignment.template.AlignedSequence;
import org.biojava.nbio.core.alignment.template.SequencePair;

/**
 * Summary-only declaration of the external BioJava Core implementation.
 * Constructor bodies are deliberately absent: this dependency is represented
 * by generated external facades, while the implements edge supplies Java
 * overload resolution with the real SimpleSequencePair -> SequencePair
 * relationship.
 */
public class SimpleSequencePair<S, C> implements SequencePair<S, C> {
    public SimpleSequencePair(AlignedSequence<S, C> query, AlignedSequence<S, C> target) {}

    public SimpleSequencePair(
            S query,
            S target,
            List<AlignedSequence.Step> querySteps,
            List<AlignedSequence.Step> targetSteps) {}

    public SimpleSequencePair(
            S query,
            S target,
            List<AlignedSequence.Step> querySteps,
            int queryStart,
            int queryEnd,
            List<AlignedSequence.Step> targetSteps,
            int targetStart,
            int targetEnd) {}
}
