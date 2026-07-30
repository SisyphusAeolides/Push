module PushService

%default total

public export
data BootStage = Kernel | PackageManager | MessageBus | Compositor | Greeter | Session

public export
data Advance : BootStage -> BootStage -> Type where
  StartCorinth : Advance Kernel PackageManager
  StartBus : Advance PackageManager MessageBus
  StartCompositor : Advance MessageBus Compositor
  StartGreeter : Advance Compositor Greeter
  StartSession : Advance Greeter Session

public export
data Running : BootStage -> Type where
  KernelRunning : Running Kernel
  CorinthRunning : Running PackageManager
  BusRunning : Running MessageBus
  CompositorRunning : Running Compositor
  GreeterRunning : Running Greeter
  SessionRunning : Running Session

public export
advance : Running before -> Advance before after -> Running after
advance KernelRunning StartCorinth = CorinthRunning
advance CorinthRunning StartBus = BusRunning
advance BusRunning StartCompositor = CompositorRunning
advance CompositorRunning StartGreeter = GreeterRunning
advance GreeterRunning StartSession = SessionRunning

public export
cosmicBoot : Running Session
cosmicBoot = advance
  (advance
    (advance
      (advance
        (advance KernelRunning StartCorinth)
        StartBus)
      StartCompositor)
    StartGreeter)
  StartSession
