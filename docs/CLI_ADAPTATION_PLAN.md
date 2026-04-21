# GFD CLI Adaptation Plan

> Claude Code CLI 아키텍처를 GFD CFD 솔버용 대화형 CLI로 적용하는 계획서

## 1. 소스 분석 요약 (claude-code-main)

### 핵심 아키텍처

```
claude-code-main/src/
├── main.tsx              # CLI 엔트리포인트 (Ink React CLI)
├── setup.ts              # 초기화 (cwd, git, hooks)
├── QueryEngine.ts        # 대화 상태 머신 (메시지 + 도구 + API)
├── query.ts              # Claude API 호출 + 도구 오케스트레이션
├── Tool.ts               # 도구 타입 정의 (schema + execute + permissions)
├── tools.ts              # 70+ 도구 레지스트리
├── Task.ts               # 장기 실행 태스크 (bash, agent, monitor)
├── commands.ts           # 80+ 슬래시 명령어 레지스트리
├── context.ts            # 시스템/사용자 컨텍스트 (memoized)
├── history.ts            # 명령 히스토리 (JSONL)
├── cost-tracker.ts       # 비용 추적 (토큰, USD)
├── ink.ts                # Ink (React CLI) 래퍼
├── replLauncher.tsx       # REPL 런처
└── dialogLaunchers.tsx    # 대화형 다이얼로그
```

### 재사용 가능한 핵심 패턴

| 패턴 | 원본 | GFD 적용 |
|------|------|----------|
| **Tool 시스템** | `Tool.ts` — schema + execute + permissions | 솔버/메시/후처리 도구 정의 |
| **Task 관리** | `Task.ts` — 디스크 백업 출력, 재개 가능 | 장기 실행 시뮬레이션 관리 |
| **Query Engine** | `QueryEngine.ts` — 메시지 + 도구 루프 | AI 보조 CFD 워크플로우 |
| **Command 레지스트리** | `commands.ts` — 슬래시 명령어 | `/mesh`, `/solve`, `/plot` 등 |
| **Context 주입** | `context.ts` — memoized 시스템 정보 | 솔버 버전, GPU 상태 |
| **Cost 추적** | `cost-tracker.ts` — 모델별 비용 | GPU 시간, 라이선스 비용 |
| **History** | `history.ts` — JSONL 로그 | 시뮬레이션 히스토리 |
| **Permission 시스템** | 도구별 권한 체크 | 파일 접근, 솔버 실행 권한 |

---

## 2. GFD CLI 구조 설계

### 2.1 디렉토리 구조

```
gfd-cli/
├── src/
│   ├── main.tsx                  # CLI 엔트리포인트
│   ├── setup.ts                  # 초기화 (솔버 감지, GPU 체크)
│   ├── GfdEngine.ts              # 핵심 엔진 (QueryEngine 대응)
│   ├── context.ts                # 시스템 컨텍스트 (솔버, GPU, 메시 라이브러리)
│   ├── history.ts                # 시뮬레이션 히스토리
│   ├── cost-tracker.ts           # GPU 시간 / 라이선스 비용 추적
│   │
│   ├── tools/                    # CFD 도구 (Tool.ts 패턴)
│   │   ├── index.ts              # 도구 레지스트리
│   │   ├── MeshGenerateTool.ts   # 메시 생성
│   │   ├── MeshImportTool.ts     # 메시 임포트 (Gmsh, OpenFOAM)
│   │   ├── MeshQualityTool.ts    # 메시 품질 검사
│   │   ├── SolverRunTool.ts      # 솔버 실행 (SIMPLE/PISO)
│   │   ├── SolverStatusTool.ts   # 솔버 상태 모니터링
│   │   ├── SolverStopTool.ts     # 솔버 중단
│   │   ├── FieldExportTool.ts    # 필드 데이터 내보내기 (VTK)
│   │   ├── ResidualPlotTool.ts   # 잔차 플롯 (터미널)
│   │   ├── BenchmarkTool.ts      # 벤치마크 실행
│   │   ├── CaseSetupTool.ts      # 케이스 설정 (JSON → 솔버 입력)
│   │   ├── BoundaryTool.ts       # 경계조건 편집
│   │   ├── MaterialTool.ts       # 물성치 설정
│   │   ├── GpuInfoTool.ts        # GPU 상태 조회
│   │   └── ParallelTool.ts       # MPI 병렬 설정
│   │
│   ├── commands/                 # 슬래시 명령어
│   │   ├── index.ts              # 명령어 레지스트리
│   │   ├── mesh.ts               # /mesh (메시 생성/임포트/검사)
│   │   ├── solve.ts              # /solve (시작/중단/재개)
│   │   ├── status.ts             # /status (잔차, 진행률)
│   │   ├── plot.ts               # /plot (터미널 차트)
│   │   ├── export.ts             # /export (VTK, CSV, OpenFOAM)
│   │   ├── benchmark.ts          # /benchmark (성능 테스트)
│   │   ├── case.ts               # /case (케이스 관리)
│   │   ├── gpu.ts                # /gpu (GPU 상태)
│   │   ├── cost.ts               # /cost (비용 요약)
│   │   ├── history.ts            # /history (시뮬레이션 기록)
│   │   └── help.ts               # /help
│   │
│   ├── tasks/                    # 장기 실행 태스크
│   │   ├── SolverTask.ts         # 솔버 프로세스 관리
│   │   ├── MeshTask.ts           # 메시 생성 프로세스
│   │   └── PostProcessTask.ts    # 후처리 프로세스
│   │
│   ├── ui/                       # Ink 컴포넌트
│   │   ├── App.tsx               # 메인 앱 래퍼
│   │   ├── REPL.tsx              # 입력 루프
│   │   ├── ResidualChart.tsx     # 터미널 잔차 차트
│   │   ├── MeshProgress.tsx      # 메시 진행률
│   │   ├── SolverMonitor.tsx     # 실시간 솔버 모니터
│   │   ├── CaseTree.tsx          # 케이스 트리 표시
│   │   └── Spinner.tsx           # 로딩 스피너
│   │
│   ├── types/                    # 타입 정의
│   │   ├── tool.ts               # Tool, ToolUseContext
│   │   ├── task.ts               # TaskType, TaskState
│   │   ├── solver.ts             # SolverConfig, ResidualPoint
│   │   ├── mesh.ts               # MeshConfig, MeshQuality
│   │   └── case.ts               # CaseConfig, BoundaryCondition
│   │
│   └── utils/                    # 유틸리티
│       ├── solver-detect.ts      # 설치된 솔버 감지
│       ├── gpu-detect.ts         # CUDA/GPU 감지
│       ├── json-config.ts        # JSON 케이스 파일 파싱
│       ├── vtk-writer.ts         # VTK 출력
│       ├── terminal-chart.ts     # 터미널 차트 렌더링
│       └── process.ts            # 프로세스 스폰/관리
│
├── package.json
├── tsconfig.json
└── README.md
```

### 2.2 핵심 타입 정의

```typescript
// types/tool.ts — Claude Code의 Tool.ts 패턴 적용
export interface GfdTool {
  name: string;
  description: string;
  schema: z.ZodSchema;
  execute: (input: unknown, context: GfdToolContext) => Promise<ToolResult>;
  isEnabled?: () => boolean;
}

export interface GfdToolContext {
  cwd: string;
  caseDir: string;           // 현재 시뮬레이션 케이스 디렉토리
  solverConfig: SolverConfig;
  meshConfig: MeshConfig;
  setProgress: (jsx: ReactNode) => void;
  tasks: Record<string, TaskState>;
  addTask: (task: TaskState) => void;
}

// types/task.ts — Claude Code의 Task.ts 패턴 적용
export type GfdTaskType = 'solver' | 'mesh' | 'postprocess' | 'benchmark';

export interface GfdTaskState {
  id: string;
  type: GfdTaskType;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'killed';
  description: string;
  startTime: number;
  endTime?: number;
  outputFile: string;        // 디스크 백업 출력
  outputOffset: number;
  pid?: number;              // OS 프로세스 ID
  // Solver-specific
  currentIteration?: number;
  maxIterations?: number;
  lastResidual?: ResidualPoint;
}
```

### 2.3 핵심 엔진 (GfdEngine.ts)

```typescript
// QueryEngine.ts 패턴을 CFD 워크플로우에 적용
export class GfdEngine {
  private tools: Map<string, GfdTool>;
  private commands: Map<string, GfdCommand>;
  private tasks: Map<string, GfdTaskState>;
  private history: GfdHistory;
  private costTracker: CostTracker;

  constructor(config: GfdEngineConfig) {
    this.tools = loadTools();
    this.commands = loadCommands();
    this.tasks = new Map();
    this.history = new GfdHistory();
    this.costTracker = new CostTracker();
  }

  // 사용자 입력 처리 (REPL 또는 배치)
  async processInput(input: string): Promise<GfdResult> {
    // 슬래시 명령어 처리
    if (input.startsWith('/')) {
      return this.executeCommand(input);
    }

    // AI 보조 모드 (Claude API 연동)
    if (this.aiMode) {
      return this.queryAI(input);
    }

    // 직접 도구 실행
    return this.executeTool(input);
  }

  // 솔버 실행 (Task 패턴)
  async runSolver(config: SolverConfig): Promise<string> {
    const taskId = generateTaskId('solver');
    const task: GfdTaskState = {
      id: taskId,
      type: 'solver',
      status: 'running',
      description: `${config.method} solver (${config.maxIterations} iter)`,
      startTime: Date.now(),
      outputFile: path.join(this.caseDir, `.gfd/tasks/${taskId}.log`),
      outputOffset: 0,
    };

    this.tasks.set(taskId, task);

    // Rust 바이너리 스폰
    const proc = spawn('gfd', ['run', config.inputFile], {
      cwd: this.caseDir,
    });

    // 출력을 디스크에 스트리밍 (재개 가능)
    proc.stdout.pipe(fs.createWriteStream(task.outputFile));

    // 잔차 파싱 + 실시간 업데이트
    proc.stdout.on('data', (data) => {
      const residual = parseResidualLine(data.toString());
      if (residual) {
        task.currentIteration = residual.iteration;
        task.lastResidual = residual;
      }
    });

    return taskId;
  }
}
```

---

## 3. 적용 계획 (Phase별)

### Phase 1: 기반 구조 (Week 1-2)

| 작업 | 원본 참조 | GFD 적용 |
|------|----------|----------|
| CLI 엔트리포인트 | `main.tsx` | `gfd-cli/src/main.tsx` — Ink REPL 부트스트랩 |
| 초기화 | `setup.ts` | 솔버 감지, GPU 체크, cwd 확인 |
| 타입 정의 | `Tool.ts`, `Task.ts` | `types/tool.ts`, `types/task.ts`, `types/solver.ts` |
| 도구 레지스트리 | `tools.ts` | `tools/index.ts` — 초기 5개 도구 |
| 명령어 레지스트리 | `commands.ts` | `commands/index.ts` — 초기 5개 명령어 |

**산출물:**
- `gfd` CLI 바이너리 (npm/npx 실행 가능)
- REPL 모드: `gfd` → 대화형 프롬프트
- 배치 모드: `gfd run config.json`

### Phase 2: 핵심 도구 (Week 3-4)

| 도구 | 기능 | 원본 패턴 |
|------|------|----------|
| `MeshGenerateTool` | Rust gfd-mesh 호출 | `BashTool` + `Task` |
| `SolverRunTool` | SIMPLE/PISO 실행 | `BashTool` + 장기 Task |
| `SolverStatusTool` | 잔차 모니터링 | `TaskOutputTool` |
| `FieldExportTool` | VTK 내보내기 | `FileWriteTool` |
| `CaseSetupTool` | JSON 케이스 편집 | `FileEditTool` |

### Phase 3: UI 컴포넌트 (Week 5-6)

| 컴포넌트 | 기능 | 원본 패턴 |
|----------|------|----------|
| `ResidualChart` | 터미널 잔차 플롯 | `cli-sparkline` 라이브러리 |
| `MeshProgress` | 메시 생성 진행률 | Ink `<Box>` + `<Text>` |
| `SolverMonitor` | 실시간 iteration 표시 | Ink `useInterval` hook |
| `CaseTree` | 케이스 디렉토리 트리 | Ink `<Tree>` 컴포넌트 |

### Phase 4: AI 연동 (Week 7-8)

| 기능 | 설명 | 원본 패턴 |
|------|------|----------|
| Claude API 연동 | AI 보조 CFD 워크플로우 | `QueryEngine.ts` |
| 자연어 → 설정 | "make mesh finer" → config 변경 | `query.ts` 도구 루프 |
| 자동 최적화 | AI가 잔차 분석 → 설정 조정 | `Tool` 오케스트레이션 |
| 결과 해석 | AI가 필드 데이터 분석 | 커스텀 시스템 프롬프트 |

---

## 4. 슬래시 명령어 설계

```
/mesh generate [--type hex|tet|poly] [--size 0.1]
/mesh import <file.msh>
/mesh quality
/mesh export <format>

/solve start [--method SIMPLE|PISO] [--iterations 500]
/solve stop
/solve resume
/solve status

/case new <name>
/case open <path>
/case save
/case list

/plot residuals [--field continuity|momentum|energy]
/plot field <pressure|velocity|temperature>
/plot convergence-rate

/export vtk [--fields all|pressure,velocity]
/export openfoam
/export csv

/gpu status
/gpu benchmark

/cost summary
/cost reset

/benchmark run [--cases all|cavity|heat]
/benchmark compare <result1> <result2>

/history
/help [command]
```

---

## 5. 핵심 코드 매핑

### Claude Code → GFD CLI

| Claude Code 파일 | GFD CLI 대응 | 변경 사항 |
|-----------------|-------------|----------|
| `main.tsx` | `main.tsx` | Claude API → Rust solver 스폰 |
| `setup.ts` | `setup.ts` | git 감지 → solver/GPU 감지 |
| `QueryEngine.ts` | `GfdEngine.ts` | 대화 루프 → 시뮬레이션 루프 |
| `Tool.ts` | `types/tool.ts` | 동일 패턴 유지 |
| `tools.ts` | `tools/index.ts` | Claude 도구 → CFD 도구 |
| `Task.ts` | `types/task.ts` | 동일 패턴 + solver 필드 추가 |
| `commands.ts` | `commands/index.ts` | 슬래시 명령어 → CFD 명령어 |
| `context.ts` | `context.ts` | git → solver version + GPU |
| `history.ts` | `history.ts` | 명령 → 시뮬레이션 히스토리 |
| `cost-tracker.ts` | `cost-tracker.ts` | API 비용 → GPU 시간 비용 |
| `ink.ts` | `ui/ink.ts` | 동일 (Ink 래퍼) |
| `replLauncher.tsx` | `ui/REPL.tsx` | 동일 패턴 |

---

## 6. 기술 스택

| 계층 | 기술 | 비고 |
|------|------|------|
| CLI 프레임워크 | **Ink** (React for CLI) | Claude Code와 동일 |
| 런타임 | **Node.js 18+** | TypeScript 컴파일 |
| 솔버 백엔드 | **Rust gfd** (기존 바이너리) | stdin/stdout IPC |
| 메시 백엔드 | **Rust gfd-mesh** (기존 크레이트) | CLI 서브커맨드 |
| AI 연동 | **Anthropic SDK** | 선택적 (--ai 모드) |
| 차트 | **cli-sparkline** / **blessed-contrib** | 터미널 차트 |
| 스키마 | **Zod** | 도구 입력 검증 |
| 빌드 | **esbuild** / **tsup** | 빠른 번들링 |

---

## 7. 실행 예시

### 대화형 모드

```
$ gfd
GFD CLI v0.1.0 | Solver: gfd v0.1.0 | GPU: CUDA 12.0 (RTX 4090)

gfd> /case open examples/lid_driven_cavity.json
  Case loaded: lid_driven_cavity (20×20 mesh, Re=100)

gfd> /mesh generate --type hex --size 0.05
  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░ 90% | 8,000 cells | 12.3s
  Mesh generated: 8,000 cells, quality: ortho=0.98, skew=0.02

gfd> /solve start --method SIMPLE --iterations 500
  [Solver] SIMPLE started (500 iter, tol=1e-4)
  [Iter  50] continuity=2.3e-2  x-mom=1.1e-2  ▁▂▃▄▅▆▇
  [Iter 100] continuity=5.1e-3  x-mom=2.3e-3  ▁▂▃▄▅▆
  [Iter 200] continuity=1.2e-4  x-mom=5.4e-5  ▁▂▃▄
  [Iter 247] CONVERGED (all < 1e-4) in 12.3s

gfd> /plot residuals
  1e-1 ┤╮
  1e-2 ┤ ╰╮
  1e-3 ┤   ╰──╮
  1e-4 ┤───────╰── tol
       └─────────────
       0   50  100  200

gfd> /export vtk --fields pressure,velocity
  Exported: results/cavity_0247.vtk (8,000 cells, 2 fields)

gfd> /cost
  Session: 12.3s CPU | 0.0 GPU hours | $0.00
```

### AI 보조 모드

```
$ gfd --ai
GFD CLI v0.1.0 + Claude AI | Solver: gfd v0.1.0

gfd> 메시를 더 세밀하게 만들어줘. 현재 수렴이 느려
  [AI] 현재 메시: 8,000 cells (h=0.05m)
  [AI] 잔차 수렴 속도: 0.015 decades/iter (느림)
  [AI] 권장: 셀 크기 0.025m (약 32,000 cells)로 리파인
  [AI] 벽면 y+ 추정: 12.3 → k-ω SST에 적합

  /mesh generate --type hex --size 0.025 실행할까요? [Y/n]
```

---

## 8. 구현 우선순위

1. **즉시 구현**: CLI 엔트리포인트 + REPL + 기본 명령어 (/help, /case, /mesh)
2. **Week 1**: 도구 시스템 (Tool.ts 패턴) + SolverRunTool + MeshGenerateTool
3. **Week 2**: Task 시스템 + 잔차 모니터링 + 진행률 표시
4. **Week 3**: Export 도구 (VTK, OpenFOAM, CSV) + 터미널 차트
5. **Week 4**: History, Cost tracking, GPU 감지
6. **Week 5-6**: AI 연동 (Claude API) + 자연어 → 설정 변환
7. **Week 7-8**: 자동 최적화 루프 + 벤치마크 비교

---

## 9. 의존성

```json
{
  "dependencies": {
    "ink": "^5.0.0",
    "ink-spinner": "^5.0.0",
    "react": "^18.0.0",
    "zod": "^3.22.0",
    "@anthropic-ai/sdk": "^0.30.0",
    "cli-sparkline": "^1.0.0",
    "chalk": "^5.0.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0",
    "tsup": "^8.0.0",
    "@types/react": "^18.0.0"
  }
}
```
