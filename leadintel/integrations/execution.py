import threading

from leadintel.vendor.execution_engine.producer import JobProducer
from leadintel.vendor.execution_engine.scheduler import run_scheduler
from leadintel.vendor.execution_engine.worker import Worker


class JobProcessor:

    def __init__(self):
        self.producer = JobProducer()
        self.worker = Worker()
        self.started = False

    def register(self, name, fn):
        self.worker.register(name, fn)

    def enqueue(self, task, payload):
        self.producer.enqueue(func_name=task, args=[payload])

    def start(self):
        if self.started:
            return

        self.started = True
        threading.Thread(target=self.worker.run, daemon=True).start()
        threading.Thread(target=run_scheduler, daemon=True).start()

        print("[OK] Job system started")


job_processor = JobProcessor()

from leadintel.core.worker import process_lead
job_processor.register("process_lead", process_lead)