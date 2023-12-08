build:
	python -m build

install-dev:
	python -m pip install -e .

uninstall:
	python -m pip uninstall -y gage

clean:
	rm -rf dist build

venv: .venv/bin/activate

.venv/bin/activate:
	virtualenv --python python3.10 .venv
