# Terror Bats: Operation Hamesh 040 Adversarial Verification Campaign

- **Target Repository:** `https://github.com/matthewjameswatkins1978-cyber/hameshgui`
- **Target Baseline Commit:** `3aafbd541dc51249565199ccb7da67bc9f1e8da1` (`main`)
- **Framework Version:** `terrorbats 1.0.0-rc.1`
- **Pack Identity:** `pack:sha256:97e6a19153817a9bffe94ee7ded3ef383ba6fce5a0b03f0329e3faa9a064274c`
- **Campaign Identity:** `campaign:sha256:ba8aaa6317eb4069011c696c2bd5585458fae1056cdee172bd58d47f48cbe85d`

## Campaign Summary

| Bat ID | Claim | Oracle | Verdict | Receipt |
| :--- | :--- | :--- | :--- | :--- |
| **BAT-01** | Distinct apps cannot inherit another app's cached layout/text state | Exit code != 0 | **PROVEN** | `receipt:sha256:f5d6422ccbe44f3f6474c24e48ec74041eb5e65aa956c91088ca21956925bb66` |
| **BAT-02** | Incremental Taffy layout always reflects current application state | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:f955ac9c7cd4d1ffb4251765d939508bb98d832bb11d7e752098b19c2f98bb8e` |
| **BAT-03** | Cached text shaping preserves accurate measurements and rendered text | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:c53760750b23c6e585739fca298c4d878ebb92790d4c47b953a3cb99f10cb2ed` |
| **BAT-04** | Changes to content, font generation or style inputs invalidate every cached result | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:dbd95e6fab8c6d95864b54652d170f7519bc638eef490a41346533936be64adf` |
| **BAT-05** | Cached layout survives legitimate tree changes without corrupting node identity | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:495aaad1410f1c273efa4074a0590a0abb0957b2d59dfe00837f06e86e12ee8d` |
| **BAT-06** | Repeated updates do not cause unbounded memory growth or latency degradation | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:5bd43717bbe6dcb4f9e3eeb1ffb0901e4a082fcecc463b89df3d004bf624bbc4` |
| **BAT-07** | AI inspection & interaction describe and manipulate what is actually displayed | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:e2f434cab5cc7210f919d81f65afdae3e1b61685b4c60b4518691d0a7dcd3f95` |
| **BAT-08** | Reported performance improvements survive changing/pathological workloads | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:cf131390a9761f5b31dd6cb68cfc43477010d55237448d1b6c0ec95158b7b760` |
| **BAT-09** | Interrupting work cannot poison persistent layout or render caches | Exit code != 0 | **NOT OBSERVED** | `receipt:sha256:2ef4190d1a0f744ed63b2ae2e48ca4ae6030ae72457afa043eda705715131029` |

## Confirmed Finding: BAT-01 Cross-Application Layout Contamination

- **Original Execution:** `9e75b1c9-e348-4f12-b383-83e7893eb42d`
- **Replayed Execution:** `6bd21b5a-0620-4455-a698-7dab6ca6a91d`
- **Replay Receipt:** `receipt:sha256:54c1c007d407acfd11be489161d5c484f8467732181341ffe466f0fd73307187`
- **Mechanism:** In `crates/hamesh-layout/src/lib.rs`, `LayoutCache::compute` checks if `topology_matches`. When two distinct applications run on the same thread sharing identical component IDs and parent/child roles, `topology_matches` evaluates to `true`. However, the incremental path skips updating style on `app.root.0` (`if id == &app.root.0 { continue; }`), causing the second application to inherit the first application's root flex direction, padding, and layout sizing.

## Replaying the Campaign

```powershell
# Replay the proven defect receipt:
terrorbats replay receipt:sha256:f5d6422ccbe44f3f6474c24e48ec74041eb5e65aa956c91088ca21956925bb66

# Re-run the full pack:
terrorbats pack run verification/terrorbats/packs/hamesh-040-adversarial.yaml --repo <clean-repo-path>
```
