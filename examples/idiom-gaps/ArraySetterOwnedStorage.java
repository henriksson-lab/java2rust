class ArraySetterOwnedStorage {
    private char[] letters;
    private int writes;

    void setLetters(char[] letters) {
        this.letters = letters;
        this.writes++;
    }

    static ArraySetterOwnedStorage join(String left, String right) {
        ArraySetterOwnedStorage out = new ArraySetterOwnedStorage();
        out.setLetters((left + right).toCharArray());
        return out;
    }

    static ArraySetterOwnedStorage copy(ArraySetterOwnedStorage source) {
        ArraySetterOwnedStorage out = new ArraySetterOwnedStorage();
        out.setLetters(source.letters);
        return out;
    }
}
