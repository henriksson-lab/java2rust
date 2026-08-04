public class LabeledBreak {
    public int run(int[][] grid, int target) {
        outer:
        for (int i = 0; i < grid.length; i++) {
            for (int j = 0; j < grid[i].length; j++) {
                if (grid[i][j] == target) { break outer; }
            }
        }
        return -1;
    }
}
