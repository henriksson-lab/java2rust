public class L3 { public int run(int n){ int t=0; outer: for(int i=0;i<n;i++){ for(int j=0;j<n;j++){ if(j==1) break outer; t+=1; } } return t; } }
