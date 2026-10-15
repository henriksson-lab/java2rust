package org.biojava.nbio.core.sequence.template;

public interface Compound {
    boolean equalsIgnoreCase(Compound compound);
    String getShortName();
    String getLongName();
    String getDescription();
    Float getMolecularWeight();
}

