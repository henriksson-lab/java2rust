package org.biojava.nbio.core.alignment.template;

import org.biojava.nbio.core.sequence.template.Compound;
import org.biojava.nbio.core.sequence.template.Sequence;

public interface AlignedSequence<S extends Sequence<C>, C extends Compound> extends Sequence<C> {
    enum Step {
        COMPOUND,
        GAP
    }

    S getOriginalSequence();
}
