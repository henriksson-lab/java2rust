package idiom.gaps;

public class NullableTrimEqualsOriginal {
    public boolean legal(String sampleName) {
        return sampleName != null && !sampleName.trim().isEmpty() && sampleName.trim().equals(sampleName);
    }
}
