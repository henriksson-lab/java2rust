package parser.gaps;

class GatkEmptyStatement {
    void count(String effect) {
        if (effect.equals("FRAME_SHIFT")) {
            frameshift();
        } else if (effect.startsWith("CODON")) {
            inFrame();
        } else
            ;
    }

    void frameshift() {
    }

    void inFrame() {
    }
}

